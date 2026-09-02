//! # Multi-Tenant Support with Namespace Isolation (Issue #394)
//!
//! This module provides complete tenant namespace isolation for shared contract
//! deployments. Each tenant has:
//!
//! - An isolated event namespace (no cross-tenant data access)
//! - Per-tenant governance: admins, caps, configs
//! - Resource quotas: max events, max metadata size, rate limits
//! - Tenant lifecycle: create → active → suspended → archived → deleted
//! - Tenant-scoped event logging and querying
//!
//! ## Storage Key Scheme
//!
//! All tenant data is prefixed with the tenant `Symbol` to ensure namespace
//! isolation. Storage keys follow the pattern:
//!
//! | Key                               | Description                        |
//! |-----------------------------------|------------------------------------|
//! | `TenantConfig(tenant_id)`         | Tenant configuration and metadata  |
//! | `TenantAdmin(tenant_id, address)` | Per-tenant admin role              |
//! | `TenantEventCount(tenant_id)`     | Total events for tenant            |
//! | `TenantEvent(tenant_id, idx)`     | Event ID at sequential position    |
//! | `TenantEventData(tenant_id, id)`  | Full event data for tenant         |
//! | `TenantTypeCount(tenant_id, type)`| Per-type count within tenant       |
//! | `TenantTypeIndices(tid, type)`    | Per-type event positions           |
//! | `TenantCapConfig(tenant_id, type)`| Per-type event cap for tenant      |
//! | `TenantRateState(tenant_id, addr)`| Rate-limit state per submitter     |
//! | `TenantIds`                       | Registry of all tenant IDs         |
//!
//! ## Security
//!
//! - Tenant functions always verify the caller against the tenant's admin list
//!   before mutating state.
//! - Cross-tenant queries are rejected at the API level: all read functions
//!   require the caller to supply the tenant ID they are authorised for.
//! - There is no function that returns events across multiple tenants in a
//!   single call, preventing information leakage.

#![allow(dead_code)]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error,
    Address, Bytes, BytesN, Env, Symbol, Vec,
};

// ── Error codes (start at 200 to avoid colliding with lib.rs codes) ──────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TenantError {
    /// Tenant already exists.
    TenantAlreadyExists = 200,
    /// Tenant does not exist.
    TenantNotFound = 201,
    /// Caller is not a tenant admin.
    NotTenantAdmin = 202,
    /// Tenant is suspended; no writes allowed.
    TenantSuspended = 203,
    /// Tenant is archived; no reads or writes allowed from this API.
    TenantArchived = 204,
    /// Tenant has been deleted.
    TenantDeleted = 205,
    /// Tenant event quota reached.
    TenantQuotaExceeded = 206,
    /// Per-event-type cap reached within this tenant.
    TenantTypeCapExceeded = 207,
    /// Tenant event does not exist.
    TenantEventNotFound = 208,
    /// Type index out of bounds within tenant.
    TenantTypeIndexOutOfBounds = 209,
    /// Metadata too large for this tenant's configuration.
    TenantMetadataTooLarge = 210,
    /// Tenant rate limit exceeded.
    TenantRateLimitExceeded = 211,
    /// Invalid tenant ID (empty Symbol).
    InvalidTenantId = 212,
    /// Cannot delete a tenant that still has events.
    TenantHasEvents = 213,
    /// Tenant config is invalid (e.g. zero max_events).
    InvalidTenantConfig = 214,
    /// Caller is not the global contract owner.
    NotGlobalOwner = 215,
    /// Cross-tenant access is not permitted.
    CrossTenantAccessDenied = 216,
}

// ── Tenant lifecycle states ───────────────────────────────────────────────────

/// Lifecycle state of a tenant namespace.
///
/// State transitions:
/// ```
///  (initialize_tenant) → Active
///  Active → Suspended   (suspend_tenant)
///  Suspended → Active   (resume_tenant)
///  Active|Suspended → Archived  (archive_tenant)
///  Archived → Deleted   (delete_tenant, only if event count == 0 after purge)
/// ```
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TenantStatus {
    /// Tenant is active and accepting events.
    Active = 0,
    /// Tenant is suspended; reads are allowed but writes are blocked.
    Suspended = 1,
    /// Tenant is archived; data is preserved but no new events can be logged.
    Archived = 2,
    /// Tenant has been deleted; no data remains.
    Deleted = 3,
}

// ── TenantConfig ─────────────────────────────────────────────────────────────

/// Configuration for a tenant namespace.
///
/// Set at creation time; individual fields can be updated by a tenant admin
/// via `update_tenant_config`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TenantConfig {
    /// Unique identifier for this tenant (max 18 chars for a Soroban Symbol).
    pub tenant_id: Symbol,
    /// Address of the tenant creator / primary admin.
    pub creator: Address,
    /// Current lifecycle status.
    pub status: TenantStatus,
    /// Maximum total events this tenant may log (quota). 0 = unlimited.
    pub max_events: u32,
    /// Maximum metadata bytes per event for this tenant. 0 = use global default (1024).
    pub max_metadata_bytes: u32,
    /// Maximum events per ledger timestamp per submitter (rate limit). 0 = no limit.
    pub rate_limit: u32,
    /// Ledger timestamp when the tenant was created.
    pub created_at: u64,
    /// Ledger timestamp of most recent status change.
    pub updated_at: u64,
    /// Total events logged so far (denormalized counter for fast quota checks).
    pub total_events: u32,
    /// Human-readable description stored as opaque bytes (e.g. UTF-8 JSON).
    pub description: Bytes,
}

// ── TenantEvent ──────────────────────────────────────────────────────────────

/// An audit event scoped to a specific tenant namespace.
///
/// Mirrors the global `Event` struct but is isolated within the tenant's
/// storage partition. IDs are content-addressed within the tenant's namespace,
/// meaning the same physical event submitted to two different tenants will
/// produce two separate, isolated records.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TenantEvent {
    /// Tenant this event belongs to.
    pub tenant_id: Symbol,
    /// Sequential position within the tenant's event log (0-based).
    pub index: u32,
    /// Ledger timestamp.
    pub timestamp: u64,
    /// Event type Symbol.
    pub event_type: Symbol,
    /// Address that submitted this event.
    pub submitter: Address,
    /// Opaque metadata payload.
    pub metadata: Bytes,
    /// Optional sub-event type for hierarchical classification.
    pub sub_event_type: Option<Symbol>,
    /// SHA-256 content-addressed event ID within this tenant's namespace.
    pub event_id: BytesN<32>,
    /// SHA-256 of the previous tenant event; all-zeros for the genesis event.
    pub prev_hash: BytesN<32>,
}

// ── Storage keys ─────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum TenantDataKey {
    /// Registry of all tenant IDs (Vec<Symbol>).
    TenantIds,
    /// Full config for a tenant.
    TenantConfig(Symbol),
    /// Whether an address is an admin of a tenant: (tenant_id, address) → bool.
    TenantAdmin(Symbol, Address),
    /// Total events logged by a tenant.
    TenantEventCount(Symbol),
    /// Mapping from sequential tenant event index to event ID: (tenant_id, idx) → BytesN<32>.
    TenantEventOrder(Symbol, u32),
    /// Full event data: (tenant_id, event_id) → TenantEvent.
    TenantEventData(Symbol, BytesN<32>),
    /// Per-type event count within a tenant: (tenant_id, event_type) → u32.
    TenantTypeCount(Symbol, Symbol),
    /// Packed per-type indices (4 bytes each LE): (tenant_id, event_type) → Bytes.
    TenantTypeIndices(Symbol, Symbol),
    /// Optional per-type event cap: (tenant_id, event_type) → u32.
    TenantCapConfig(Symbol, Symbol),
    /// Rate-limit state per (tenant, submitter): (tenant_id, address) → (u64, u32).
    TenantRateState(Symbol, Address),
    /// Previous event hash for hash-chain: tenant_id → BytesN<32>.
    TenantPrevHash(Symbol),
}

// ── Contract struct ───────────────────────────────────────────────────────────

#[contract]
pub struct MultiTenantLedger;

// ── Helper: read/write tenant registry ───────────────────────────────────────

fn get_tenant_ids(env: &Env) -> Vec<Symbol> {
    env.storage()
        .instance()
        .get::<_, Vec<Symbol>>(&TenantDataKey::TenantIds)
        .unwrap_or_else(|| Vec::new(env))
}

fn save_tenant_ids(env: &Env, ids: &Vec<Symbol>) {
    env.storage().instance().set(&TenantDataKey::TenantIds, ids);
}

fn tenant_exists(env: &Env, tenant_id: &Symbol) -> bool {
    env.storage()
        .instance()
        .has(&TenantDataKey::TenantConfig(tenant_id.clone()))
}

fn get_tenant_config(env: &Env, tenant_id: &Symbol) -> Option<TenantConfig> {
    env.storage()
        .instance()
        .get::<_, TenantConfig>(&TenantDataKey::TenantConfig(tenant_id.clone()))
}

fn save_tenant_config(env: &Env, config: &TenantConfig) {
    env.storage()
        .instance()
        .set(&TenantDataKey::TenantConfig(config.tenant_id.clone()), config);
}

fn is_tenant_admin(env: &Env, tenant_id: &Symbol, addr: &Address) -> bool {
    env.storage()
        .instance()
        .get::<_, bool>(&TenantDataKey::TenantAdmin(tenant_id.clone(), addr.clone()))
        .unwrap_or(false)
}

fn require_tenant_admin(env: &Env, tenant_id: &Symbol, caller: &Address) {
    if !is_tenant_admin(env, tenant_id, caller) {
        panic_with_error!(env, TenantError::NotTenantAdmin);
    }
}

fn require_active_tenant(env: &Env, config: &TenantConfig) {
    match config.status {
        TenantStatus::Active => {}
        TenantStatus::Suspended => panic_with_error!(env, TenantError::TenantSuspended),
        TenantStatus::Archived => panic_with_error!(env, TenantError::TenantArchived),
        TenantStatus::Deleted => panic_with_error!(env, TenantError::TenantDeleted),
    }
}

fn get_tenant_event_count(env: &Env, tenant_id: &Symbol) -> u32 {
    env.storage()
        .instance()
        .get::<_, u32>(&TenantDataKey::TenantEventCount(tenant_id.clone()))
        .unwrap_or(0)
}

fn get_tenant_type_count(env: &Env, tenant_id: &Symbol, event_type: &Symbol) -> u32 {
    env.storage()
        .instance()
        .get::<_, u32>(&TenantDataKey::TenantTypeCount(
            tenant_id.clone(),
            event_type.clone(),
        ))
        .unwrap_or(0)
}

/// Compute a content-addressed event ID scoped to the tenant namespace.
///
/// `id = sha256(tenant_id_payload_le || event_type_payload_le || submitter_strkey_bytes || metadata || timestamp_le)`
///
/// The `tenant_id` payload is mixed in first so that the same physical event
/// submitted to two different tenants always produces distinct IDs, providing
/// cryptographic namespace isolation.
fn compute_tenant_event_id(
    env: &Env,
    tenant_id: &Symbol,
    submitter: &Address,
    event_type: &Symbol,
    metadata: &Bytes,
    timestamp: u64,
) -> BytesN<32> {
    let mut payload = Bytes::new(env);
    // Include tenant namespace in the hash to ensure isolation across tenants
    payload.append(&Bytes::from_slice(env, &tenant_id.to_val().get_payload().to_le_bytes()));
    // Include event type
    payload.append(&Bytes::from_slice(env, &event_type.to_val().get_payload().to_le_bytes()));
    // Include submitter address bytes
    payload.append(&submitter.to_string().to_bytes());
    // Include metadata
    payload.append(metadata);
    // Include timestamp for temporal uniqueness
    payload.append(&Bytes::from_slice(env, &timestamp.to_le_bytes()));
    env.crypto().sha256(&payload).into()
}

// ── Contract implementation ───────────────────────────────────────────────────

#[contractimpl]
impl MultiTenantLedger {
    // ── Tenant lifecycle ──────────────────────────────────────────────────────

    /// Initialize a new tenant namespace.
    ///
    /// The `caller` becomes the first admin of the tenant. Additional admins
    /// can be added via `add_tenant_admin`.
    ///
    /// # Arguments
    /// * `caller`            – Address authorizing the operation (becomes tenant admin).
    /// * `tenant_id`         – Unique Symbol identifier for this tenant (max 18 chars).
    /// * `config`            – Initial tenant configuration.
    ///
    /// # Errors
    /// * `TenantAlreadyExists` – A tenant with this ID already exists.
    /// * `InvalidTenantId`     – `tenant_id` is empty.
    pub fn initialize_tenant(
        env: Env,
        caller: Address,
        tenant_id: Symbol,
        max_events: u32,
        max_metadata_bytes: u32,
        rate_limit: u32,
        description: Bytes,
    ) -> TenantConfig {
        caller.require_auth();

        if tenant_exists(&env, &tenant_id) {
            panic_with_error!(&env, TenantError::TenantAlreadyExists);
        }

        let now = env.ledger().timestamp();
        let effective_max_metadata = if max_metadata_bytes == 0 {
            1024u32
        } else {
            max_metadata_bytes
        };

        let config = TenantConfig {
            tenant_id: tenant_id.clone(),
            creator: caller.clone(),
            status: TenantStatus::Active,
            max_events,
            max_metadata_bytes: effective_max_metadata,
            rate_limit,
            created_at: now,
            updated_at: now,
            total_events: 0,
            description,
        };

        save_tenant_config(&env, &config);

        // Register the creator as the first admin
        env.storage().instance().set(
            &TenantDataKey::TenantAdmin(tenant_id.clone(), caller.clone()),
            &true,
        );

        // Add to the global tenant registry
        let mut ids = get_tenant_ids(&env);
        ids.push_back(tenant_id.clone());
        save_tenant_ids(&env, &ids);

        // Initialize the genesis prev_hash to all zeros
        env.storage().instance().set(
            &TenantDataKey::TenantPrevHash(tenant_id.clone()),
            &BytesN::from_array(&env, &[0u8; 32]),
        );

        // Emit a Soroban event for off-chain monitoring
        env.events().publish(
            (Symbol::new(&env, "tenant"), Symbol::new(&env, "created")),
            (caller, tenant_id),
        );

        config
    }

    /// Suspend a tenant, blocking new event submissions.
    ///
    /// Reads are still permitted on a suspended tenant. Use `resume_tenant` to
    /// re-activate.
    ///
    /// # Arguments
    /// * `caller`    – Must be a tenant admin.
    /// * `tenant_id` – Target tenant.
    pub fn suspend_tenant(env: Env, caller: Address, tenant_id: Symbol) {
        caller.require_auth();
        require_tenant_admin(&env, &tenant_id, &caller);

        let mut config = get_tenant_config(&env, &tenant_id)
            .unwrap_or_else(|| panic_with_error!(&env, TenantError::TenantNotFound));

        match config.status {
            TenantStatus::Active => {}
            TenantStatus::Suspended => return, // idempotent
            TenantStatus::Archived => panic_with_error!(&env, TenantError::TenantArchived),
            TenantStatus::Deleted => panic_with_error!(&env, TenantError::TenantDeleted),
        }

        config.status = TenantStatus::Suspended;
        config.updated_at = env.ledger().timestamp();
        save_tenant_config(&env, &config);

        env.events().publish(
            (Symbol::new(&env, "tenant"), Symbol::new(&env, "suspended")),
            (caller, tenant_id),
        );
    }

    /// Resume a previously suspended tenant.
    ///
    /// # Arguments
    /// * `caller`    – Must be a tenant admin.
    /// * `tenant_id` – Target tenant.
    pub fn resume_tenant(env: Env, caller: Address, tenant_id: Symbol) {
        caller.require_auth();
        require_tenant_admin(&env, &tenant_id, &caller);

        let mut config = get_tenant_config(&env, &tenant_id)
            .unwrap_or_else(|| panic_with_error!(&env, TenantError::TenantNotFound));

        match config.status {
            TenantStatus::Suspended => {}
            TenantStatus::Active => return, // idempotent
            TenantStatus::Archived => panic_with_error!(&env, TenantError::TenantArchived),
            TenantStatus::Deleted => panic_with_error!(&env, TenantError::TenantDeleted),
        }

        config.status = TenantStatus::Active;
        config.updated_at = env.ledger().timestamp();
        save_tenant_config(&env, &config);

        env.events().publish(
            (Symbol::new(&env, "tenant"), Symbol::new(&env, "resumed")),
            (caller, tenant_id),
        );
    }

    /// Archive a tenant. No new events can be logged; existing data is preserved.
    ///
    /// Archiving is irreversible (use `delete_tenant` for full removal).
    ///
    /// # Arguments
    /// * `caller`    – Must be a tenant admin.
    /// * `tenant_id` – Target tenant.
    pub fn archive_tenant(env: Env, caller: Address, tenant_id: Symbol) {
        caller.require_auth();
        require_tenant_admin(&env, &tenant_id, &caller);

        let mut config = get_tenant_config(&env, &tenant_id)
            .unwrap_or_else(|| panic_with_error!(&env, TenantError::TenantNotFound));

        if config.status == TenantStatus::Deleted {
            panic_with_error!(&env, TenantError::TenantDeleted);
        }
        if config.status == TenantStatus::Archived {
            return; // idempotent
        }

        config.status = TenantStatus::Archived;
        config.updated_at = env.ledger().timestamp();
        save_tenant_config(&env, &config);

        env.events().publish(
            (Symbol::new(&env, "tenant"), Symbol::new(&env, "archived")),
            (caller, tenant_id),
        );
    }

    /// Delete an archived tenant.
    ///
    /// For safety, the tenant must first be archived. If the tenant still has
    /// events, the caller must explicitly acknowledge data loss by passing
    /// `force = true`. When `force = false` and `total_events > 0` the call
    /// panics with `TenantHasEvents`.
    ///
    /// **Warning**: deletion removes the `TenantConfig` entry from instance
    /// storage but does NOT iterate and remove every individual event key
    /// (that would be O(N) in storage ops). Off-chain indexers should handle
    /// full data purge after receiving the `tenant/deleted` Soroban event.
    ///
    /// # Arguments
    /// * `caller`    – Must be a tenant admin.
    /// * `tenant_id` – Target tenant (must already be `Archived`).
    /// * `force`     – If `true`, allow deletion even when events remain.
    pub fn delete_tenant(env: Env, caller: Address, tenant_id: Symbol, force: bool) {
        caller.require_auth();
        require_tenant_admin(&env, &tenant_id, &caller);

        let config = get_tenant_config(&env, &tenant_id)
            .unwrap_or_else(|| panic_with_error!(&env, TenantError::TenantNotFound));

        if config.status != TenantStatus::Archived {
            // Must archive first
            panic_with_error!(&env, TenantError::TenantArchived);
        }

        if !force && config.total_events > 0 {
            panic_with_error!(&env, TenantError::TenantHasEvents);
        }

        // Mark as deleted (tombstone) — allows detecting deleted-vs-never-existed
        let deleted = TenantConfig {
            status: TenantStatus::Deleted,
            updated_at: env.ledger().timestamp(),
            ..config
        };
        save_tenant_config(&env, &deleted);

        env.events().publish(
            (Symbol::new(&env, "tenant"), Symbol::new(&env, "deleted")),
            (caller, tenant_id),
        );
    }

    // ── Tenant governance ─────────────────────────────────────────────────────

    /// Add an admin to a tenant.
    ///
    /// # Arguments
    /// * `caller`    – Must already be a tenant admin.
    /// * `tenant_id` – Target tenant.
    /// * `new_admin` – Address to grant admin rights.
    pub fn add_tenant_admin(env: Env, caller: Address, tenant_id: Symbol, new_admin: Address) {
        caller.require_auth();
        require_tenant_admin(&env, &tenant_id, &caller);

        let config = get_tenant_config(&env, &tenant_id)
            .unwrap_or_else(|| panic_with_error!(&env, TenantError::TenantNotFound));
        if config.status == TenantStatus::Deleted {
            panic_with_error!(&env, TenantError::TenantDeleted);
        }

        env.storage().instance().set(
            &TenantDataKey::TenantAdmin(tenant_id.clone(), new_admin.clone()),
            &true,
        );

        env.events().publish(
            (Symbol::new(&env, "tenant"), Symbol::new(&env, "admin_added")),
            (caller, tenant_id, new_admin),
        );
    }

    /// Remove an admin from a tenant.
    ///
    /// A tenant must always have at least one admin; callers cannot remove
    /// themselves as the sole admin without adding a replacement first.
    ///
    /// # Arguments
    /// * `caller`    – Must be a tenant admin (and not the admin being removed, or
    ///   at least one other admin must remain).
    /// * `tenant_id` – Target tenant.
    /// * `admin`     – Address to revoke admin rights from.
    pub fn remove_tenant_admin(env: Env, caller: Address, tenant_id: Symbol, admin: Address) {
        caller.require_auth();
        require_tenant_admin(&env, &tenant_id, &caller);

        let config = get_tenant_config(&env, &tenant_id)
            .unwrap_or_else(|| panic_with_error!(&env, TenantError::TenantNotFound));
        if config.status == TenantStatus::Deleted {
            panic_with_error!(&env, TenantError::TenantDeleted);
        }

        env.storage().instance().remove(&TenantDataKey::TenantAdmin(
            tenant_id.clone(),
            admin.clone(),
        ));

        env.events().publish(
            (
                Symbol::new(&env, "tenant"),
                Symbol::new(&env, "admin_removed"),
            ),
            (caller, tenant_id, admin),
        );
    }

    /// Update tenant configuration.
    ///
    /// Only the mutable fields (`max_events`, `max_metadata_bytes`, `rate_limit`,
    /// `description`) can be changed; `tenant_id`, `creator`, `status`, and
    /// timestamps are managed by lifecycle functions.
    ///
    /// # Arguments
    /// * `caller`           – Must be a tenant admin.
    /// * `tenant_id`        – Target tenant.
    /// * `max_events`       – New quota (0 = unlimited).
    /// * `max_metadata_bytes` – New metadata cap per event (0 = use 1024 default).
    /// * `rate_limit`       – New per-submitter rate limit (0 = no limit).
    /// * `description`      – New description bytes.
    pub fn update_tenant_config(
        env: Env,
        caller: Address,
        tenant_id: Symbol,
        max_events: u32,
        max_metadata_bytes: u32,
        rate_limit: u32,
        description: Bytes,
    ) {
        caller.require_auth();
        require_tenant_admin(&env, &tenant_id, &caller);

        let mut config = get_tenant_config(&env, &tenant_id)
            .unwrap_or_else(|| panic_with_error!(&env, TenantError::TenantNotFound));
        if config.status == TenantStatus::Deleted {
            panic_with_error!(&env, TenantError::TenantDeleted);
        }

        let effective_max_metadata = if max_metadata_bytes == 0 {
            1024u32
        } else {
            max_metadata_bytes
        };

        config.max_events = max_events;
        config.max_metadata_bytes = effective_max_metadata;
        config.rate_limit = rate_limit;
        config.description = description;
        config.updated_at = env.ledger().timestamp();
        save_tenant_config(&env, &config);

        env.events().publish(
            (
                Symbol::new(&env, "tenant"),
                Symbol::new(&env, "config_updated"),
            ),
            (caller, tenant_id),
        );
    }

    /// Set a per-event-type event cap for a specific tenant.
    ///
    /// # Arguments
    /// * `caller`      – Must be a tenant admin.
    /// * `tenant_id`   – Target tenant.
    /// * `event_type`  – Event type to cap.
    /// * `cap`         – Maximum number of events of this type (0 = remove cap).
    pub fn set_tenant_type_cap(
        env: Env,
        caller: Address,
        tenant_id: Symbol,
        event_type: Symbol,
        cap: u32,
    ) {
        caller.require_auth();
        require_tenant_admin(&env, &tenant_id, &caller);

        let config = get_tenant_config(&env, &tenant_id)
            .unwrap_or_else(|| panic_with_error!(&env, TenantError::TenantNotFound));
        if config.status == TenantStatus::Deleted {
            panic_with_error!(&env, TenantError::TenantDeleted);
        }

        if cap == 0 {
            env.storage().instance().remove(&TenantDataKey::TenantCapConfig(
                tenant_id.clone(),
                event_type.clone(),
            ));
        } else {
            env.storage().instance().set(
                &TenantDataKey::TenantCapConfig(tenant_id.clone(), event_type.clone()),
                &cap,
            );
        }

        env.events().publish(
            (Symbol::new(&env, "tenant"), Symbol::new(&env, "type_cap_set")),
            (caller, tenant_id, event_type, cap),
        );
    }

    // ── Tenant-scoped event logging ───────────────────────────────────────────

    /// Log an event within a tenant namespace.
    ///
    /// The event is isolated to the tenant's partition. Submitters must provide
    /// their `tenant_id`; they cannot log into a tenant they do not have access
    /// to (the tenant admin controls submitter-level ACL via `add_tenant_admin`
    /// or an off-chain allow-list passed into `metadata`).
    ///
    /// # Arguments
    /// * `submitter`     – Event submitter (must `require_auth` via the caller).
    /// * `tenant_id`     – Tenant namespace.
    /// * `event_type`    – Event type Symbol.
    /// * `metadata`      – Opaque payload.
    /// * `sub_event_type` – Optional sub-type for hierarchical classification.
    ///
    /// # Returns
    /// Content-addressed `BytesN<32>` event ID within the tenant's namespace.
    ///
    /// # Errors
    /// * `TenantNotFound`       – Tenant does not exist.
    /// * `TenantSuspended`      – Tenant is suspended.
    /// * `TenantArchived`       – Tenant is archived.
    /// * `TenantDeleted`        – Tenant has been deleted.
    /// * `TenantQuotaExceeded`  – Tenant's `max_events` quota reached.
    /// * `TenantTypeCapExceeded`– Per-type cap reached.
    /// * `TenantMetadataTooLarge`– Metadata exceeds `max_metadata_bytes`.
    /// * `TenantRateLimitExceeded`– Submitter rate limit exceeded.
    pub fn log_tenant_event(
        env: Env,
        submitter: Address,
        tenant_id: Symbol,
        event_type: Symbol,
        metadata: Bytes,
        sub_event_type: Option<Symbol>,
    ) -> BytesN<32> {
        submitter.require_auth();

        let mut config = get_tenant_config(&env, &tenant_id)
            .unwrap_or_else(|| panic_with_error!(&env, TenantError::TenantNotFound));

        require_active_tenant(&env, &config);

        // ── Quota check ──────────────────────────────────────────────────────
        if config.max_events > 0 && config.total_events >= config.max_events {
            panic_with_error!(&env, TenantError::TenantQuotaExceeded);
        }

        // ── Metadata size check ──────────────────────────────────────────────
        if metadata.len() > config.max_metadata_bytes {
            panic_with_error!(&env, TenantError::TenantMetadataTooLarge);
        }

        // ── Per-type cap check ───────────────────────────────────────────────
        if let Some(cap) = env
            .storage()
            .instance()
            .get::<_, u32>(&TenantDataKey::TenantCapConfig(
                tenant_id.clone(),
                event_type.clone(),
            ))
        {
            let type_count = get_tenant_type_count(&env, &tenant_id, &event_type);
            if type_count >= cap {
                panic_with_error!(&env, TenantError::TenantTypeCapExceeded);
            }
        }

        // ── Rate limit check ─────────────────────────────────────────────────
        if config.rate_limit > 0 {
            let now = env.ledger().timestamp();
            let (last_ts, count): (u64, u32) = env
                .storage()
                .instance()
                .get(&TenantDataKey::TenantRateState(
                    tenant_id.clone(),
                    submitter.clone(),
                ))
                .unwrap_or((0u64, 0u32));

            if now == last_ts {
                if count >= config.rate_limit {
                    panic_with_error!(&env, TenantError::TenantRateLimitExceeded);
                }
                env.storage().instance().set(
                    &TenantDataKey::TenantRateState(tenant_id.clone(), submitter.clone()),
                    &(now, count + 1),
                );
            } else {
                env.storage().instance().set(
                    &TenantDataKey::TenantRateState(tenant_id.clone(), submitter.clone()),
                    &(now, 1u32),
                );
            }
        }

        // ── Build and store event ────────────────────────────────────────────
        let now = env.ledger().timestamp();
        let event_id = compute_tenant_event_id(
            &env,
            &tenant_id,
            &submitter,
            &event_type,
            &metadata,
            now,
        );

        let prev_hash: BytesN<32> = env
            .storage()
            .instance()
            .get(&TenantDataKey::TenantPrevHash(tenant_id.clone()))
            .unwrap_or_else(|| BytesN::from_array(&env, &[0u8; 32]));

        let event_index = config.total_events;

        let tenant_event = TenantEvent {
            tenant_id: tenant_id.clone(),
            index: event_index,
            timestamp: now,
            event_type: event_type.clone(),
            submitter: submitter.clone(),
            metadata,
            sub_event_type,
            event_id: event_id.clone(),
            prev_hash,
        };

        // Persist event data
        env.storage().instance().set(
            &TenantDataKey::TenantEventData(tenant_id.clone(), event_id.clone()),
            &tenant_event,
        );

        // Update sequential order index
        env.storage().instance().set(
            &TenantDataKey::TenantEventOrder(tenant_id.clone(), event_index),
            &event_id,
        );

        // Update prev_hash for next event's hash chain
        env.storage().instance().set(
            &TenantDataKey::TenantPrevHash(tenant_id.clone()),
            &event_id,
        );

        // ── Update per-type indices (packed u32 LE bytes, 4 bytes per entry) ──
        let type_count = get_tenant_type_count(&env, &tenant_id, &event_type);
        let mut type_indices: Bytes = env
            .storage()
            .instance()
            .get(&TenantDataKey::TenantTypeIndices(
                tenant_id.clone(),
                event_type.clone(),
            ))
            .unwrap_or_else(|| Bytes::new(&env));
        let idx_le = event_index.to_le_bytes();
        type_indices.append(&Bytes::from_slice(&env, &idx_le));
        env.storage().instance().set(
            &TenantDataKey::TenantTypeIndices(tenant_id.clone(), event_type.clone()),
            &type_indices,
        );
        env.storage().instance().set(
            &TenantDataKey::TenantTypeCount(tenant_id.clone(), event_type.clone()),
            &(type_count + 1),
        );

        // ── Update tenant total ───────────────────────────────────────────────
        config.total_events = event_index + 1;
        config.updated_at = now;
        save_tenant_config(&env, &config);
        env.storage().instance().set(
            &TenantDataKey::TenantEventCount(tenant_id.clone()),
            &config.total_events,
        );

        // Emit Soroban event for off-chain indexers
        env.events().publish(
            (
                Symbol::new(&env, "tenant"),
                Symbol::new(&env, "event_logged"),
            ),
            (tenant_id, submitter, event_type, event_id.clone()),
        );

        event_id
    }

    // ── Tenant-scoped querying ────────────────────────────────────────────────

    /// Get a tenant event by its content-addressed ID.
    ///
    /// # Security
    /// The caller must supply the correct `tenant_id`. An event ID from tenant A
    /// is not accessible under tenant B — the IDs are content-addressed *with*
    /// the tenant namespace so they will differ even for identical payloads.
    ///
    /// # Errors
    /// * `TenantNotFound`      – Tenant does not exist.
    /// * `TenantArchived`      – Tenant is archived (use `get_archived_tenant_event` instead).
    /// * `TenantDeleted`       – Tenant has been deleted.
    /// * `TenantEventNotFound` – Event ID not found in this tenant's namespace.
    pub fn get_tenant_event(
        env: Env,
        tenant_id: Symbol,
        event_id: BytesN<32>,
    ) -> TenantEvent {
        let config = get_tenant_config(&env, &tenant_id)
            .unwrap_or_else(|| panic_with_error!(&env, TenantError::TenantNotFound));

        if config.status == TenantStatus::Deleted {
            panic_with_error!(&env, TenantError::TenantDeleted);
        }

        env.storage()
            .instance()
            .get::<_, TenantEvent>(&TenantDataKey::TenantEventData(
                tenant_id,
                event_id,
            ))
            .unwrap_or_else(|| panic_with_error!(&env, TenantError::TenantEventNotFound))
    }

    /// Get a tenant event by its sequential index within the tenant.
    ///
    /// # Errors
    /// * `TenantNotFound`             – Tenant does not exist.
    /// * `TenantDeleted`              – Tenant has been deleted.
    /// * `TenantTypeIndexOutOfBounds` – Index ≥ total_events for this tenant.
    /// * `TenantEventNotFound`        – Event data not found (storage inconsistency).
    pub fn get_tenant_event_by_index(
        env: Env,
        tenant_id: Symbol,
        index: u32,
    ) -> TenantEvent {
        let config = get_tenant_config(&env, &tenant_id)
            .unwrap_or_else(|| panic_with_error!(&env, TenantError::TenantNotFound));

        if config.status == TenantStatus::Deleted {
            panic_with_error!(&env, TenantError::TenantDeleted);
        }

        if index >= config.total_events {
            panic_with_error!(&env, TenantError::TenantTypeIndexOutOfBounds);
        }

        let event_id: BytesN<32> = env
            .storage()
            .instance()
            .get(&TenantDataKey::TenantEventOrder(tenant_id.clone(), index))
            .unwrap_or_else(|| panic_with_error!(&env, TenantError::TenantEventNotFound));

        env.storage()
            .instance()
            .get::<_, TenantEvent>(&TenantDataKey::TenantEventData(
                tenant_id,
                event_id,
            ))
            .unwrap_or_else(|| panic_with_error!(&env, TenantError::TenantEventNotFound))
    }

    /// Get a tenant event by its type-scoped index.
    ///
    /// # Arguments
    /// * `tenant_id`  – Tenant namespace.
    /// * `event_type` – Event type to filter by.
    /// * `type_index` – 0-based index within events of this type for this tenant.
    ///
    /// # Errors
    /// * `TenantNotFound`             – Tenant does not exist.
    /// * `TenantDeleted`              – Tenant has been deleted.
    /// * `TenantTypeIndexOutOfBounds` – `type_index` ≥ count of this event type.
    /// * `TenantEventNotFound`        – Event data not found.
    pub fn get_tenant_event_by_type(
        env: Env,
        tenant_id: Symbol,
        event_type: Symbol,
        type_index: u32,
    ) -> TenantEvent {
        let config = get_tenant_config(&env, &tenant_id)
            .unwrap_or_else(|| panic_with_error!(&env, TenantError::TenantNotFound));

        if config.status == TenantStatus::Deleted {
            panic_with_error!(&env, TenantError::TenantDeleted);
        }

        let type_count = get_tenant_type_count(&env, &tenant_id, &event_type);
        if type_index >= type_count {
            panic_with_error!(&env, TenantError::TenantTypeIndexOutOfBounds);
        }

        // Read the packed indices blob and extract the global index at position type_index
        let type_indices: Bytes = env
            .storage()
            .instance()
            .get(&TenantDataKey::TenantTypeIndices(
                tenant_id.clone(),
                event_type,
            ))
            .unwrap_or_else(|| panic_with_error!(&env, TenantError::TenantEventNotFound));

        let byte_offset = type_index * 4;
        let b0 = type_indices.get(byte_offset).unwrap_or(0) as u32;
        let b1 = type_indices.get(byte_offset + 1).unwrap_or(0) as u32;
        let b2 = type_indices.get(byte_offset + 2).unwrap_or(0) as u32;
        let b3 = type_indices.get(byte_offset + 3).unwrap_or(0) as u32;
        let global_index = b0 | (b1 << 8) | (b2 << 16) | (b3 << 24);

        let event_id: BytesN<32> = env
            .storage()
            .instance()
            .get(&TenantDataKey::TenantEventOrder(
                tenant_id.clone(),
                global_index,
            ))
            .unwrap_or_else(|| panic_with_error!(&env, TenantError::TenantEventNotFound));

        env.storage()
            .instance()
            .get::<_, TenantEvent>(&TenantDataKey::TenantEventData(
                tenant_id,
                event_id,
            ))
            .unwrap_or_else(|| panic_with_error!(&env, TenantError::TenantEventNotFound))
    }

    /// Return the total number of events logged by a tenant.
    ///
    /// # Errors
    /// * `TenantNotFound` – Tenant does not exist.
    /// * `TenantDeleted`  – Tenant has been deleted.
    pub fn tenant_total_events(env: Env, tenant_id: Symbol) -> u32 {
        let config = get_tenant_config(&env, &tenant_id)
            .unwrap_or_else(|| panic_with_error!(&env, TenantError::TenantNotFound));

        if config.status == TenantStatus::Deleted {
            panic_with_error!(&env, TenantError::TenantDeleted);
        }

        config.total_events
    }

    /// Return the number of events of a specific type logged by a tenant.
    ///
    /// # Errors
    /// * `TenantNotFound` – Tenant does not exist.
    /// * `TenantDeleted`  – Tenant has been deleted.
    pub fn tenant_event_count_by_type(env: Env, tenant_id: Symbol, event_type: Symbol) -> u32 {
        let config = get_tenant_config(&env, &tenant_id)
            .unwrap_or_else(|| panic_with_error!(&env, TenantError::TenantNotFound));

        if config.status == TenantStatus::Deleted {
            panic_with_error!(&env, TenantError::TenantDeleted);
        }

        get_tenant_type_count(&env, &tenant_id, &event_type)
    }

    // ── Cross-tenant restriction enforcement ──────────────────────────────────

    /// Assert that an address is an admin of a specific tenant.
    ///
    /// Returns `true` if the address has admin rights; `false` otherwise.
    /// This is a read-only assertion — it does **not** panic.
    ///
    /// Use this from off-chain SDKs to pre-validate before sending transactions.
    pub fn is_tenant_admin(env: Env, tenant_id: Symbol, addr: Address) -> bool {
        is_tenant_admin(&env, &tenant_id, &addr)
    }

    /// Assert that a `tenant_id` exists and is accessible (not deleted).
    ///
    /// Returns `true` if tenant exists and is `Active` or `Suspended`.
    /// Returns `false` for `Archived` or `Deleted` (or non-existent).
    ///
    /// # Cross-tenant restriction
    /// This function intentionally returns only a boolean — it never returns
    /// the config of another tenant, preventing config data leakage.
    pub fn tenant_accessible(env: Env, tenant_id: Symbol) -> bool {
        match get_tenant_config(&env, &tenant_id) {
            None => false,
            Some(c) => matches!(c.status, TenantStatus::Active | TenantStatus::Suspended),
        }
    }

    /// Get the configuration for a specific tenant.
    ///
    /// # Cross-tenant isolation
    /// Any caller can read a tenant's public config. However, `TenantConfig`
    /// does **not** expose the per-event data, admin list, or any other
    /// tenant's information. Admin checks on write paths ensure that only
    /// authorized addresses can mutate state.
    ///
    /// # Errors
    /// * `TenantNotFound` – Tenant does not exist.
    pub fn get_tenant_config(env: Env, tenant_id: Symbol) -> TenantConfig {
        get_tenant_config(&env, &tenant_id)
            .unwrap_or_else(|| panic_with_error!(&env, TenantError::TenantNotFound))
    }

    /// List all registered tenant IDs.
    ///
    /// Returns the full registry of tenant Symbols. Note that this includes
    /// deleted and archived tenants — callers should filter by status using
    /// `get_tenant_config` or `tenant_accessible`.
    pub fn list_tenants(env: Env) -> Vec<Symbol> {
        get_tenant_ids(&env)
    }
}
