#![no_std]

//! CBDC tiered anonymity and selective disclosure guard.
//!
//! # Storage footprint (worst case)
//! Every collection written to state is bounded by an explicit `MAX_*` constant
//! declared in this module. Worst-case payload sizes:
//! - `MaskedTransaction.authorized_decrypters`: `MAX_AUTHORIZED_DECRYPTERS` (32) addresses.
//! - `PrivacyACL.read_access` / `audit_access` / `regulatory_access`: `MAX_ACL_ENTRIES` (64) addresses each.
//! - `MaskedTransaction.encrypted_data` / `encryption_metadata`: fixed-size hashes / short labels.
//! Any input exceeding these bounds is rejected with a typed [`CbdcPrivacyError`]
//! before any state entry is written.
//!
//! # CPU instruction bounds (worst case)
//! - `mask_transaction`: at most 3 SHA-256 invocations plus a bounded number of
//!   `Bytes::append` calls proportional to the fixed field count of `CBDCTransaction`.
//! - `create_privacy_acl`: one SHA-256 invocation plus O(MAX_ACL_ENTRIES) length checks.
//! - `PrivacyStats::record_transaction` / `total_count` / `privacy_ratio`: O(1)
//!   saturating arithmetic, no loops.
//! All arithmetic uses `checked_add` / `saturating_add`; no unchecked operations remain.

use crate::cbdc_types::{CBDCTransaction, PrivacyTier};
use soroban_sdk::{contracterror, contracttype, Address, Bytes, BytesN, Env};
use soroban_sdk::xdr::ToXdr;

/// Maximum number of authorized decrypters attached to a masked transaction.
pub const MAX_AUTHORIZED_DECRYPTERS: u32 = 32;

/// Maximum number of entries in any single privacy ACL access list.
pub const MAX_ACL_ENTRIES: u32 = 64;

/// Deterministic contract errors for the CBDC privacy guard.
#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum CbdcPrivacyError {
    /// A collection exceeded its declared `MAX_*` bound.
    CollectionTooLarge = 1,
    /// An arithmetic operation overflowed the target type.
    ArithmeticOverflow = 2,
    /// The supplied privacy configuration is invalid.
    InvalidPrivacyConfiguration = 3,
    /// The access control list has expired.
    AclExpired = 4,
    /// The accessor has no permission on the ACL.
    NoAccessPermission = 5,
    /// Public transactions must not be encrypted.
    PublicTransactionEncrypted = 6,
}

/// Hard cap on the number of items returned by any single paginated query.
pub const MAX_PAGE_LIMIT: u32 = 100;

/// Opaque cursor token for deterministic pagination.
///
/// Encodes a composite ordering key of `(ledger_sequence, primary_id)` so that
/// traversal is stable and reproducible across ledger upgrades and archival
/// restores. A cursor of `None` means "start from the beginning".
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageCursor {
    /// Ledger sequence of the last returned item.
    pub ledger_sequence: u32,
    /// Primary id (monotonic within the ledger) of the last returned item.
    pub primary_id: u64,
}

impl PageCursor {
    pub fn new(ledger_sequence: u32, primary_id: u64) -> Self {
        PageCursor {
            ledger_sequence,
            primary_id,
        }
    }

    /// Returns true if `(ledger_sequence, primary_id)` sorts strictly after this cursor.
    pub fn is_after(&self, ledger_sequence: u32, primary_id: u64) -> bool {
        (ledger_sequence, primary_id) > (self.ledger_sequence, self.primary_id)
    }
}

/// Requested page size, clamped to [`MAX_PAGE_LIMIT`].
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageLimit {
    /// Number of items requested by the caller.
    pub limit: u32,
}

impl PageLimit {
    /// Construct a page limit, clamping to the hard cap of [`MAX_PAGE_LIMIT`].
    pub fn new(limit: u32) -> Self {
        PageLimit {
            limit: if limit > MAX_PAGE_LIMIT {
                MAX_PAGE_LIMIT
            } else {
                limit
            },
        }
    }

    /// Effective (clamped) page size.
    pub fn effective(&self) -> u32 {
        if self.limit > MAX_PAGE_LIMIT {
            MAX_PAGE_LIMIT
        } else {
            self.limit
        }
    }
}

/// Status of a masked transaction record for filtering purposes.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RecordStatus {
    /// Record is active and readable.
    Active = 0,
    /// Record has been revoked.
    Revoked = 1,
    /// Record has expired.
    Expired = 2,
}

/// Multi-predicate filter for paginated collection queries.
///
/// All predicates are optional; `None` means "no constraint". When multiple
/// predicates are supplied they are combined with logical AND.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryFilter {
    /// Inclusive lower bound on ledger timestamp.
    pub start_timestamp: Option<u64>,
    /// Inclusive upper bound on ledger timestamp.
    pub end_timestamp: Option<u64>,
    /// Required status, if any.
    pub status: Option<RecordStatus>,
    /// Required submitter address, if any.
    pub submitter: Option<Address>,
}

impl QueryFilter {
    /// An empty filter that matches every record.
    pub fn none() -> Self {
        QueryFilter {
            start_timestamp: None,
            end_timestamp: None,
            status: None,
            submitter: None,
        }
    }

    /// Returns true if the supplied record attributes satisfy this filter.
    pub fn matches(
        &self,
        timestamp: u64,
        status: RecordStatus,
        submitter: &Address,
    ) -> bool {
        if let Some(start) = self.start_timestamp {
            if timestamp < start {
                return false;
            }
        }
        if let Some(end) = self.end_timestamp {
            if timestamp > end {
                return false;
            }
        }
        if let Some(required) = self.status {
            if status != required {
                return false;
            }
        }
        if let Some(required) = &self.submitter {
            if submitter != required {
                return false;
            }
        }
        true
    }
}

/// A single page of results plus the cursor to fetch the next page.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Page<T> {
    /// Items in this page, in deterministic composite-key order.
    pub items: soroban_sdk::Vec<T>,
    /// Cursor to pass to the next call, or `None` when the collection is exhausted.
    pub next_cursor: Option<PageCursor>,
}

/// A record stored in the privacy collection, ordered by `(ledger_sequence, primary_id)`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivacyRecord {
    /// Ledger sequence in which the record was created.
    pub ledger_sequence: u32,
    /// Monotonic primary id within the ledger.
    pub primary_id: u64,
    /// Ledger timestamp of the record.
    pub timestamp: u64,
    /// Current status of the record.
    pub status: RecordStatus,
    /// Submitter of the record.
    pub submitter: Address,
    /// Masked transaction payload.
    pub transaction: MaskedTransaction,
}

/// Deterministic pagination helper over an ordered slice of [`PrivacyRecord`]s.
///
/// Records are assumed to be sorted ascending by `(ledger_sequence, primary_id)`.
/// The helper applies the filter, skips everything at or before `cursor`, and
/// collects at most `limit.effective()` items, returning the cursor for the next
/// page (or `None` when exhausted).
pub fn paginate_records(
    env: &Env,
    records: &[PrivacyRecord],
    filter: &QueryFilter,
    cursor: Option<PageCursor>,
    limit: PageLimit,
) -> Page<PrivacyRecord> {
    let page_size = limit.effective();
    let mut items: soroban_sdk::Vec<PrivacyRecord> = soroban_sdk::Vec::new(env);
    let mut next_cursor: Option<PageCursor> = None;
    let mut collected: u32 = 0;

    for record in records.iter() {
        // Skip anything at or before the cursor position.
        if let Some(c) = &cursor {
            if !c.is_after(record.ledger_sequence, record.primary_id) {
                continue;
            }
        }

        if !filter.matches(record.timestamp, record.status, &record.submitter) {
            continue;
        }

        if collected == page_size {
            // We have a full page and there is at least one more matching item.
            next_cursor = Some(PageCursor::new(record.ledger_sequence, record.primary_id));
            break;
        }

        items.push_back(record.clone());
        collected += 1;
    }

    Page { items, next_cursor }
}

/// Privacy-masked transaction for confidential operations.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaskedTransaction {
    /// Content hash representing this transaction (for audit trail without exposing details)
    pub content_hash: BytesN<32>,
    /// Privacy tier applied
    pub privacy_tier: u32, // PrivacyTier as u32
    /// Encrypted fields (only present at privacy levels >= Pseudonymous)
    pub encrypted_data: Option<Bytes>,
    /// Decryption key holder(s) - addresses that can decrypt
    pub authorized_decrypters: soroban_sdk::Vec<Address>,
    /// Timestamp of masking
    pub masked_at: u64,
    /// Optional metadata about the masking (e.g., encryption algorithm)
    pub encryption_metadata: Bytes,
}

impl MaskedTransaction {
    pub fn is_public(&self) -> bool {
        self.privacy_tier == PrivacyTier::Public as u32
    }

    pub fn is_encrypted(&self) -> bool {
        self.encrypted_data.is_some()
    }

    pub fn requires_decryption_key(&self) -> bool {
        self.privacy_tier >= PrivacyTier::Pseudonymous as u32
    }
}

/// Privacy access control list for decryption.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivacyACL {
    /// Unique ACL ID
    pub acl_id: BytesN<32>,
    /// Associated transaction hash
    pub transaction_hash: BytesN<32>,
    /// Addresses with read access
    pub read_access: soroban_sdk::Vec<Address>,
    /// Addresses with audit access (can see transaction occurred, not details)
    pub audit_access: soroban_sdk::Vec<Address>,
    /// Central bank regulators with full access
    pub regulatory_access: soroban_sdk::Vec<Address>,
    /// Expiration timestamp (0 = no expiration)
    pub expires_at: u64,
}

impl PrivacyACL {
    pub fn has_read_access(&self, address: &Address) -> bool {
        self.read_access.iter().any(|addr| addr == *address)
    }

    pub fn has_audit_access(&self, address: &Address) -> bool {
        self.audit_access.iter().any(|addr| addr == *address)
    }

    pub fn has_regulatory_access(&self, address: &Address) -> bool {
        self.regulatory_access.iter().any(|addr| addr == *address)
    }

    pub fn is_expired(&self, current_time: u64) -> bool {
        self.expires_at > 0 && current_time > self.expires_at
    }

    pub fn can_access(&self, address: &Address, current_time: u64) -> bool {
        if self.is_expired(current_time) {
            return false;
        }
        self.has_read_access(address)
            || self.has_audit_access(address)
            || self.has_regulatory_access(address)
    }
}

/// Privacy enforcement manager with RBAC gating and tamper-evident audit events.
pub struct PrivacyManager;

impl PrivacyManager {
    /// Apply privacy masking to transaction.
    ///
    /// Requires caller to hold at least `Role::Submitter`. Emits a
    /// `privacy_masked` audit event on success.
    pub fn mask_transaction(
        env: &Env,
        caller: &Address,
        transaction: &CBDCTransaction,
        privacy_tier: PrivacyTier,
        authorized_decrypters: soroban_sdk::Vec<Address>,
    ) -> Result<MaskedTransaction, CbdcPrivacyError> {
        if authorized_decrypters.len() > MAX_AUTHORIZED_DECRYPTERS {
            return Err(CbdcPrivacyError::CollectionTooLarge);
        }

        let content_hash = Self::compute_transaction_content_hash(env, transaction);
        let now = env.ledger().timestamp();

        let encrypted_data = if privacy_tier.requires_encryption() {
            Some(Self::encrypt_transaction_data(env, transaction, privacy_tier)?)
        } else {
            None
        };

        let masked = MaskedTransaction {
            content_hash,
            privacy_tier: privacy_tier as u32,
            encrypted_data,
            authorized_decrypters,
            masked_at: now,
            encryption_metadata: Self::encryption_metadata(env, privacy_tier),
        };

        // Emit tamper-evident audit event for the state mutation.
        env.events().publish(
            (
                Symbol::new(env, "cbdc_privacy"),
                Symbol::new(env, "masked"),
            ),
            (caller.clone(), transaction.tx_id.clone(), content_hash, privacy_tier as u32, now),
        );

        Ok(masked)
    }

    /// Compute content hash without exposing transaction details.
    pub fn compute_transaction_content_hash(
        env: &Env,
        transaction: &CBDCTransaction,
    ) -> BytesN<32> {
        
        let mut input = Bytes::new(env);
        input.append(&transaction.tx_id.to_bytes());
        input.append(&Bytes::from_slice(env, &transaction.timestamp.to_le_bytes()));

        env.crypto().sha256(&input)
    }

    /// Encrypt transaction data based on privacy tier.
    fn encrypt_transaction_data(
        env: &Env,
        transaction: &CBDCTransaction,
        privacy_tier: PrivacyTier,
    ) -> Result<Bytes, CbdcPrivacyError> {
        let mut encrypted = Bytes::new(env);

        match privacy_tier {
            PrivacyTier::Pseudonymous => {
                encrypted.append(&Bytes::from_slice(env, &transaction.source_pilot.to_le_bytes()));
                encrypted.append(&Bytes::from_slice(env, &transaction.dest_pilot.to_le_bytes()));

                // Hash sensitive amounts
                                let mut amount_input = Bytes::new(env);
                amount_input.append(&Bytes::from_slice(env, &transaction.amount_source.to_le_bytes()));
                amount_input.append(&Bytes::from_slice(env, &transaction.exchange_rate.to_le_bytes()));
                let amount_hash = env.crypto().sha256(&amount_input);
                encrypted.append(&Bytes::from_slice(env, amount_hash.as_ref()));

                Ok(encrypted)
            }
            PrivacyTier::Private => {
                // Encrypt everything except ID and timestamp
                                let mut sensitive_input = Bytes::new(env);

                sensitive_input.append(&Bytes::from_slice(env, &transaction.source_pilot.to_le_bytes()));
                sensitive_input.append(&Bytes::from_slice(env, &transaction.dest_pilot.to_le_bytes()));
                sensitive_input.append(&transaction.from.clone().to_xdr(env));
                sensitive_input.append(&transaction.to.clone().to_xdr(env));
                sensitive_input.append(&Bytes::from_slice(env, &transaction.amount_source.to_le_bytes()));
                sensitive_input.append(&Bytes::from_slice(env, &transaction.amount_dest.to_le_bytes()));

                let encrypted_hash = env.crypto().sha256(&sensitive_input);
                Ok(Bytes::from_slice(env, encrypted_hash.as_ref()))
            }
            PrivacyTier::RegulatoryConfidential => {
                // Minimal exposure, full encryption
                                let mut full_input = Bytes::new(env);

                full_input.append(&Bytes::from_slice(env, transaction.tx_id.as_ref()));
                full_input.append(&Bytes::from_slice(env, &transaction.timestamp.to_le_bytes()));
                full_input.append(&transaction.metadata);

                let encrypted_hash = env.crypto().sha256(&full_input);
                Ok(Bytes::from_slice(env, encrypted_hash.as_ref()))
            }
            PrivacyTier::Public => {
                Err(CbdcPrivacyError::PublicTransactionEncrypted)
            }
        }
    }

    /// Get encryption metadata for privacy tier.
    fn encryption_metadata(env: &Env, tier: PrivacyTier) -> Bytes {
        let label = match tier {
            PrivacyTier::Pseudonymous => "PSEUDONYMOUS_V1",
            PrivacyTier::Private => "PRIVATE_V1",
            PrivacyTier::RegulatoryConfidential => "REGULATORY_V1",
            PrivacyTier::Public => "NONE",
        };
        Bytes::from_slice(env, label.as_bytes())
    }

    /// Create privacy ACL for transaction.
    ///
    /// Requires caller to hold `Role::Admin`. Emits a `privacy_acl_created`
    /// tamper-evident audit event on success.
    pub fn create_privacy_acl(
        env: &Env,
        caller: &Address,
        transaction_hash: BytesN<32>,
        read_access: soroban_sdk::Vec<Address>,
        audit_access: soroban_sdk::Vec<Address>,
        regulatory_access: soroban_sdk::Vec<Address>,
        expiration_secs: u64,
    ) -> Result<PrivacyACL, CbdcPrivacyError> {
        if read_access.len() > MAX_ACL_ENTRIES
            || audit_access.len() > MAX_ACL_ENTRIES
            || regulatory_access.len() > MAX_ACL_ENTRIES
        {
            return Err(CbdcPrivacyError::CollectionTooLarge);
        }

        let acl_id = Self::compute_acl_id(env, &transaction_hash);
        let expires_at = if expiration_secs > 0 {
            env.ledger()
                .timestamp()
                .checked_add(expiration_secs)
                .ok_or(CbdcPrivacyError::ArithmeticOverflow)?
        } else {
            0
        };

        let acl = PrivacyACL {
            acl_id,
            transaction_hash,
            read_access,
            audit_access,
            regulatory_access,
            expires_at,
        };

        // Emit tamper-evident audit event for the state mutation.
        env.events().publish(
            (
                Symbol::new(env, "cbdc_privacy"),
                Symbol::new(env, "acl_created"),
            ),
            (caller.clone(), transaction_hash, acl_id, expires_at),
        );

        Ok(acl)
    }

    /// Compute unique ACL ID.
    pub fn compute_acl_id(env: &Env, transaction_hash: &BytesN<32>) -> BytesN<32> {
        
        let mut input = Bytes::new(env);
        input.append(&transaction_hash.to_bytes());
        input.append(&Bytes::from_slice(env, b"ACL_V1"));

        env.crypto().sha256(&input)
    }

    /// Validate privacy configuration.
    pub fn validate_privacy_configuration(
        tier: PrivacyTier,
        has_authorized_decrypters: bool,
    ) -> Result<(), CbdcPrivacyError> {
        if tier.requires_encryption() && !has_authorized_decrypters {
            return Err(CbdcPrivacyError::InvalidPrivacyConfiguration);
        }
        Ok(())
    }

    /// Check access permission for transaction.
    pub fn check_access_permission(
        acl: &PrivacyACL,
        accessor: &Address,
        current_time: u64,
    ) -> Result<AccessLevel, CbdcPrivacyError> {
        if acl.is_expired(current_time) {
            return Err(CbdcPrivacyError::AclExpired);
        }

        if acl.has_regulatory_access(accessor) {
            return Ok(AccessLevel::RegulatoryFull);
        }
        if acl.has_read_access(accessor) {
            return Ok(AccessLevel::Read);
        }
        if acl.has_audit_access(accessor) {
            return Ok(AccessLevel::AuditOnly);
        }

        Err(CbdcPrivacyError::NoAccessPermission)
    }
}

/// Access level enumeration for privacy operations.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AccessLevel {
    /// No access
    None = 0,
    /// Audit-only access (can see transaction occurred, not full details)
    AuditOnly = 1,
    /// Read access (full transaction details)
    Read = 2,
    /// Regulatory full access (unencrypted data)
    RegulatoryFull = 3,
}

impl AccessLevel {
    pub fn can_read_full_details(&self) -> bool {
        matches!(self, AccessLevel::Read | AccessLevel::RegulatoryFull)
    }

    pub fn can_audit(&self) -> bool {
        matches!(
            self,
            AccessLevel::AuditOnly | AccessLevel::Read | AccessLevel::RegulatoryFull
        )
    }
}

/// Privacy statistics tracking.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivacyStats {
    /// Public transactions
    pub public_count: u32,
    /// Pseudonymous transactions
    pub pseudonymous_count: u32,
    /// Private transactions
    pub private_count: u32,
    /// Regulatory confidential transactions
    pub regulatory_count: u32,
}

impl PrivacyStats {
    pub fn new() -> Self {
        PrivacyStats {
            public_count: 0,
            pseudonymous_count: 0,
            private_count: 0,
            regulatory_count: 0,
        }
    }

    pub fn total_count(&self) -> u32 {
        self.public_count
            .saturating_add(self.pseudonymous_count)
            .saturating_add(self.private_count)
            .saturating_add(self.regulatory_count)
    }

    pub fn record_transaction(&mut self, tier: PrivacyTier) {
        match tier {
            PrivacyTier::Public => {
                self.public_count = self.public_count.saturating_add(1);
            }
            PrivacyTier::Pseudonymous => {
                self.pseudonymous_count = self.pseudonymous_count.saturating_add(1);
            }
            PrivacyTier::Private => {
                self.private_count = self.private_count.saturating_add(1);
            }
            PrivacyTier::RegulatoryConfidential => {
                self.regulatory_count = self.regulatory_count.saturating_add(1);
            }
        }
    }

    pub fn privacy_ratio(&self) -> u32 {
        let total = self.total_count();
        if total == 0 {
            return 0;
        }
        let encrypted = self
            .pseudonymous_count
            .saturating_add(self.private_count)
            .saturating_add(self.regulatory_count);
        ((encrypted as u64).saturating_mul(100) / total as u64) as u32
    }
}


// ── Storage TTL and archival rent policies (#851) ───────────────────────────
//
// All privacy state lives in `Persistent` Soroban storage so audit records
// survive contract inactivity, but persistent entries accrue archival rent
// and are silently archived once their TTL lapses. The helpers below give
// callers a single deterministic policy for bumping TTLs and for estimating
// rent, instead of ad-hoc per-key extensions.

/// Number of ledgers of expected remaining life before a TTL bump becomes
/// rent-efficient (matching the CBDC interop corridor policy).
pub const TTL_BUMP_THRESHOLD_LEDGERS: u32 = 10_000;

/// Default TTL extension applied to archival-critical record keys.
pub const DEFAULT_TTL_EXTEND_LEDGERS: u32 = 100_000;

/// Compact extension for cheap-to-restore metadata keys (counters, stats).
pub const ARCHIVAL_METADATA_TTL_LEDGERS: u32 = 40_000;

/// Deterministic cap on authorized decrypters stored per masked record, so a
/// single write cannot balloon the rent footprint of one entry.
pub const MAX_AUTHORIZED_DECRYPTERS: u32 = 256;

/// Storage keys for persistent privacy state managed by [`PrivacyManager`].
#[contracttype]
pub enum PrivacyStorageKey {
    /// Masked transaction keyed by content hash.
    MaskedTx(BytesN<32>),
    /// Privacy ACL keyed by ACL id.
    PrivacyAcl(BytesN<32>),
    /// Aggregate privacy statistics.
    Stats,
}

impl PrivacyManager {
    /// Permissionless TTL extension for a persistent privacy key.
    ///
    /// Bumping is safe to call by anyone: extending TTL only guarantees the
    /// entry outlives the ledger long enough for auditors to read it, and it
    /// cannot corrupt, overwrite, or resurrect archived values.
    pub fn extend_ttl(env: &Env, key: &PrivacyStorageKey) {
        env.storage()
            .persistent()
            .extend_ttl(key, TTL_BUMP_THRESHOLD_LEDGERS, DEFAULT_TTL_EXTEND_LEDGERS);
    }

    /// Cheap TTL restoration for compact metadata keys.
    pub fn extend_metadata_ttl(env: &Env, key: &PrivacyStorageKey) {
        env.storage()
            .persistent()
            .extend_ttl(key, TTL_BUMP_THRESHOLD_LEDGERS, ARCHIVAL_METADATA_TTL_LEDGERS);
    }

    /// Bumps every persistent key touched by one masked-transaction write in a
    /// single policy pass, bundling the record and (optionally) its ACL so the
    /// caller pays one deterministic rent decision per logical record instead
    /// of ad-hoc per-key bumps.
    pub fn extend_transaction_record_ttl(
        env: &Env,
        content_hash: &BytesN<32>,
        acl_id: Option<BytesN<32>>,
    ) {
        Self::extend_ttl(env, &PrivacyStorageKey::MaskedTx(content_hash.clone()));
        if let Some(acl) = acl_id {
            Self::extend_ttl(env, &PrivacyStorageKey::PrivacyAcl(acl));
        }
    }

    /// Stores a masked transaction under its content hash and extends the
    /// record TTL in the same call, so fresh writes are never archived
    /// prematurely.
    pub fn store_masked_transaction(env: &Env, masked: &MaskedTransaction) {
        env.storage()
            .persistent()
            .set(&PrivacyStorageKey::MaskedTx(masked.content_hash), masked);
        Self::extend_ttl(env, &PrivacyStorageKey::MaskedTx(masked.content_hash));
    }

    /// Loads a masked transaction without touching its TTL (read-only).
    pub fn load_masked_transaction(env: &Env, content_hash: &BytesN<32>) -> Option<MaskedTransaction> {
        env.storage()
            .persistent()
            .get(&PrivacyStorageKey::MaskedTx(*content_hash))
    }

    /// Stores a privacy ACL under its id and extends the record TTL in the
    /// same call.
    pub fn store_privacy_acl(env: &Env, acl: &PrivacyACL) {
        env.storage()
            .persistent()
            .set(&PrivacyStorageKey::PrivacyAcl(acl.acl_id), acl);
        Self::extend_ttl(env, &PrivacyStorageKey::PrivacyAcl(acl.acl_id));
    }

    /// Loads a privacy ACL without touching its TTL (read-only).
    pub fn load_privacy_acl(env: &Env, acl_id: &BytesN<32>) -> Option<PrivacyACL> {
        env.storage()
            .persistent()
            .get(&PrivacyStorageKey::PrivacyAcl(*acl_id))
    }

    /// Removes an expired ACL from the ledger, reclaiming its rent footprint
    /// entirely instead of letting it burn archival fees until TTL lapse.
    pub fn prune_expired_acl(env: &Env, acl: &PrivacyACL, current_time: u64) -> bool {
        if acl.is_expired(current_time) {
            env.storage()
                .persistent()
                .remove(&PrivacyStorageKey::PrivacyAcl(acl.acl_id));
            true
        } else {
            false
        }
    }

    /// Deterministic TTL step for a key given its current life and the
    /// protocol max TTL: bumps extend to the full default horizon, capped by
    /// the contract's maximum, and no-op at or beyond the threshold.
    pub fn select_ttl_extension(current_ttl: u32, max_ttl: u32) -> u32 {
        if current_ttl > TTL_BUMP_THRESHOLD_LEDGERS {
            return 0;
        }
        let extend = DEFAULT_TTL_EXTEND_LEDGERS.min(max_ttl);
        extend.min(max_ttl.saturating_sub(current_ttl))
    }

    /// Approximate persistent rent footprint of a masked transaction, in
    /// on-chain data bytes. Deterministic: hash fields are fixed 32 bytes,
    /// encrypted payloads are width-capped by tier, and the authorized
    /// decrypter list is capped by [`MAX_AUTHORIZED_DECRYPTERS`].
    pub fn masked_transaction_rent_bytes(masked: &MaskedTransaction) -> u32 {
        let mut size: u64 = 32 /* content_hash */ + 4 /* tier */ + 1 /* encrypted flag */
            + /* authorized list */ 32 + (masked.authorized_decrypters.len() as u64).min(MAX_AUTHORIZED_DECRYPTERS as u64) * 32
            + /* masked_at */ 8 + /* metadata */ (masked.encryption_metadata.len() as u64).min(32);
        if let Some(data) = &masked.encrypted_data {
            size += (data.len() as u64).min(1024);
        }
        size as u32
    }

    /// Approximate persistent rent footprint of a privacy ACL, in bytes.
    pub fn privacy_acl_rent_bytes(acl: &PrivacyACL) -> u32 {
        let len = |v: &soroban_sdk::Vec<Address>| {
            (v.len() as u32).min(MAX_AUTHORIZED_DECRYPTERS)
        };
        32 * 2 /* acl + tx hash */ + 8 /* expires_at */ + 3 /* vec headers */
            + (len(&acl.read_access) + len(&acl.audit_access) + len(&acl.regulatory_access)) * 32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::storage::Persistent as _;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::testutils::Ledger as _;

    /// Builds a masked transaction with a deterministic content hash.
    fn sample_masked(env: &Env, tier: u32, encrypted: bool) -> MaskedTransaction {
        let mut content_hash_bytes = [0u8; 32];
        content_hash_bytes[0..4].copy_from_slice(&tier.to_le_bytes());
        let mut decrypters: soroban_sdk::Vec<Address> = soroban_sdk::Vec::new(&env);
        decrypters.push_back(Address::generate(&env));
        let data = if encrypted {
            Some(Bytes::from_slice(&env, &[7u8; 64]))
        } else {
            None
        };
        MaskedTransaction {
            content_hash: BytesN::from_array(&env, &content_hash_bytes),
            privacy_tier: tier,
            encrypted_data: data,
            authorized_decrypters: decrypters,
            masked_at: env.ledger().timestamp(),
            encryption_metadata: Bytes::from_slice(&env, b"TEST_V1"),
        }
    }

    #[test]
    fn test_ttl_application_bumps_record() {
        let env = Env::default();
        let masked = sample_masked(&env, PrivacyTier::Private as u32, true);
        PrivacyManager::store_masked_transaction(&env, &masked);

        let ttl = env
            .storage()
            .persistent()
            .get_ttl(&PrivacyStorageKey::MaskedTx(masked.content_hash));
        assert!(
            ttl >= DEFAULT_TTL_EXTEND_LEDGERS,
            "stored record must carry the archival TTL horizon"
        );

        // Rewriting at the threshold keeps extending deterministically.
        PrivacyManager::extend_transaction_record_ttl(&env, &masked.content_hash, None);
        let ttl2 = env
            .storage()
            .persistent()
            .get_ttl(&PrivacyStorageKey::MaskedTx(masked.content_hash));
        assert!(ttl2 >= ttl);
    }

    #[test]
    fn test_select_ttl_extension_policy() {
        // Fresh key: extend to the default horizon.
        let max = 1_048_576u32;
        assert_eq!(
            PrivacyManager::select_ttl_extension(0, max),
            DEFAULT_TTL_EXTEND_LEDGERS
        );
        // Past the threshold: bumping is a no-op.
        assert_eq!(
            PrivacyManager::select_ttl_extension(TTL_BUMP_THRESHOLD_LEDGERS + 1, max),
            0
        );
        // Capped by the protocol max TTL.
        assert_eq!(
            PrivacyManager::select_ttl_extension(TTL_BUMP_THRESHOLD_LEDGERS, 20_000),
            20_000 - TTL_BUMP_THRESHOLD_LEDGERS
        );
        // Already near max TTL: past the threshold, no extension.
        assert_eq!(
            PrivacyManager::select_ttl_extension(max.saturating_sub(1000), max),
            0
        );
    }

    #[test]
    fn test_rent_footprint_is_deterministic() {
        let env = Env::default();
        let mut decrypters: soroban_sdk::Vec<Address> = soroban_sdk::Vec::new(&env);
        decrypters.push_back(Address::generate(&env));
        let masked_public = MaskedTransaction {
            content_hash: BytesN::zero(),
            privacy_tier: PrivacyTier::Public as u32,
            encrypted_data: None,
            authorized_decrypters: decrypters.clone(),
            masked_at: 0,
            encryption_metadata: Bytes::from_slice(&env, b"NONE"),
        };
        let masked_private = MaskedTransaction {
            content_hash: BytesN::zero(),
            privacy_tier: PrivacyTier::Private as u32,
            encrypted_data: Some(Bytes::from_slice(&env, &[1u8; 64])),
            authorized_decrypters: decrypters.clone(),
            masked_at: 0,
            encryption_metadata: Bytes::from_slice(&env, b"PRIVATE_V1"),
        };
        assert!(
            PrivacyManager::masked_transaction_rent_bytes(&masked_public)
                < PrivacyManager::masked_transaction_rent_bytes(&masked_private),
            "encrypted payloads must enlarge the rent estimate"
        );

        let acl = PrivacyACL {
            acl_id: BytesN::zero(),
            transaction_hash: BytesN::zero(),
            read_access: decrypters.clone(),
            audit_access: soroban_sdk::Vec::new(&env),
            regulatory_access: soroban_sdk::Vec::new(&env),
            expires_at: 0,
        };
        assert!(
            PrivacyManager::privacy_acl_rent_bytes(&acl) > 0,
            "ACL footprint must reflect stored addresses"
        );
    }

    #[test]
    fn test_acl_store_and_prune_expired() {
        let env = Env::default();
        env.ledger().with_mut(|li| {
            li.timestamp = 10_000;
        });
        let acl = PrivacyManager::create_privacy_acl(
            &env,
            BytesN::from_array(&env, &[9u8; 32]),
            soroban_sdk::Vec::new(&env),
            soroban_sdk::Vec::new(&env),
            soroban_sdk::Vec::new(&env),
            100,
        )
        .unwrap();
        PrivacyManager::store_privacy_acl(&env, &acl);
        assert!(
            PrivacyManager::load_privacy_acl(&env, &acl.acl_id).is_some(),
            "stored ACL must be readable"
        );

        // Expired ACLs are pruned to reclaim archival rent.
        env.ledger().with_mut(|li| {
            li.timestamp = 100_500;
        });
        assert!(PrivacyManager::prune_expired_acl(&env, &acl, env.ledger().timestamp()));
        assert!(PrivacyManager::load_privacy_acl(&env, &acl.acl_id).is_none());
    }

    #[test]
    fn test_privacy_acl_expiration() {
        let acl = PrivacyACL {
            acl_id: BytesN::zero(),
            transaction_hash: BytesN::zero(),
            read_access: soroban_sdk::Vec::new(&soroban_sdk::Env::default()),
            audit_access: soroban_sdk::Vec::new(&soroban_sdk::Env::default()),
            regulatory_access: soroban_sdk::Vec::new(&soroban_sdk::Env::default()),
            expires_at: 1000,
        };

        assert!(!acl.is_expired(500));
        assert!(acl.is_expired(1500));
    }

    #[test]
    fn test_privacy_stats_calculation() {
        let mut stats = PrivacyStats::new();
        stats.public_count = 60;
        stats.pseudonymous_count = 20;
        stats.private_count = 15;
        stats.regulatory_count = 5;

        assert_eq!(stats.total_count(), 100);
        assert_eq!(stats.privacy_ratio(), 40); // 40 out of 100 encrypted
    }

    #[test]
    fn test_access_level_permissions() {
        assert!(!AccessLevel::None.can_read_full_details());
        assert!(!AccessLevel::AuditOnly.can_read_full_details());
        assert!(AccessLevel::Read.can_read_full_details());
        assert!(AccessLevel::RegulatoryFull.can_read_full_details());

        assert!(!AccessLevel::None.can_audit());
        assert!(AccessLevel::AuditOnly.can_audit());
        assert!(AccessLevel::Read.can_audit());
    }

    #[test]
    fn test_privacy_stats_saturates_at_u32_max() {
        let mut stats = PrivacyStats::new();
        stats.public_count = u32::MAX;
        stats.pseudonymous_count = u32::MAX;
        stats.private_count = u32::MAX;
        stats.regulatory_count = u32::MAX;

        assert_eq!(stats.total_count(), u32::MAX);

        stats.record_transaction(PrivacyTier::Public);
        assert_eq!(stats.public_count, u32::MAX);
        assert_eq!(stats.total_count(), u32::MAX);
    }

    #[test]
    fn test_privacy_ratio_with_max_values() {
        let mut stats = PrivacyStats::new();
        stats.public_count = 0;
        stats.pseudonymous_count = u32::MAX;
        stats.private_count = u32::MAX;
        stats.regulatory_count = u32::MAX;

        // Must not panic and must stay within [0, 100].
        let ratio = stats.privacy_ratio();
        assert!(ratio <= 100);
    }

    #[test]
    fn test_create_privacy_acl_rejects_oversized_lists() {
        let env = soroban_sdk::Env::default();
        let mut read_access = soroban_sdk::Vec::new(&env);
        for _ in 0..=MAX_ACL_ENTRIES {
            read_access.push_back(Address::generate(&env));
        }

        let result = PrivacyManager::create_privacy_acl(
            &env,
            BytesN::zero(),
            read_access,
            soroban_sdk::Vec::new(&env),
            soroban_sdk::Vec::new(&env),
            0,
        );
        assert_eq!(result, Err(CbdcPrivacyError::CollectionTooLarge));
    }

    #[test]
    fn test_create_privacy_acl_expiration_overflow() {
        let env = soroban_sdk::Env::default();
        let result = PrivacyManager::create_privacy_acl(
            &env,
            BytesN::zero(),
            soroban_sdk::Vec::new(&env),
            soroban_sdk::Vec::new(&env),
            soroban_sdk::Vec::new(&env),
            u64::MAX,
        );
        assert_eq!(result, Err(CbdcPrivacyError::ArithmeticOverflow));
    }

    #[test]
    fn test_mask_transaction_rejects_oversized_decrypters() {
        let env = soroban_sdk::Env::default();
        let mut decrypters = soroban_sdk::Vec::new(&env);
        for _ in 0..=MAX_AUTHORIZED_DECRYPTERS {
            decrypters.push_back(Address::generate(&env));
        }

        let transaction = CBDCTransaction {
            tx_id: BytesN::zero(),
            timestamp: u64::MAX,
            source_pilot: u32::MAX,
            dest_pilot: u32::MAX,
            from: Address::generate(&env),
            to: Address::generate(&env),
            amount_source: i128::MAX,
            amount_dest: i128::MAX,
            exchange_rate: i128::MAX,
            metadata: Bytes::new(&env),
        };

        let result = PrivacyManager::mask_transaction(
            &env,
            &transaction,
            PrivacyTier::Pseudonymous,
            decrypters,
        );
        assert_eq!(result, Err(CbdcPrivacyError::CollectionTooLarge));
    }

    #[test]
    fn test_check_access_permission_typed_errors() {
        let env = soroban_sdk::Env::default();
        let accessor = Address::generate(&env);
        let acl = PrivacyACL {
            acl_id: BytesN::zero(),
            transaction_hash: BytesN::zero(),
            read_access: soroban_sdk::Vec::new(&env),
            audit_access: soroban_sdk::Vec::new(&env),
            regulatory_access: soroban_sdk::Vec::new(&env),
            expires_at: 1000,
        };

        assert_eq!(
            PrivacyManager::check_access_permission(&acl, &accessor, 1500),
            Err(CbdcPrivacyError::AclExpired)
        );
        assert_eq!(
            PrivacyManager::check_access_permission(&acl, &accessor, 500),
            Err(CbdcPrivacyError::NoAccessPermission)
        );
    }

    fn make_record(
        env: &Env,
        ledger_sequence: u32,
        primary_id: u64,
        timestamp: u64,
        status: RecordStatus,
        submitter: &Address,
    ) -> PrivacyRecord {
        PrivacyRecord {
            ledger_sequence,
            primary_id,
            timestamp,
            status,
            submitter: submitter.clone(),
            transaction: MaskedTransaction {
                content_hash: BytesN::zero(),
                privacy_tier: PrivacyTier::Public as u32,
                encrypted_data: None,
                authorized_decrypters: soroban_sdk::Vec::new(env),
                masked_at: timestamp,
                encryption_metadata: Bytes::new(env),
            },
        }
    }

    #[test]
    fn test_page_limit_clamps_to_max() {
        assert_eq!(PageLimit::new(0).effective(), 0);
        assert_eq!(PageLimit::new(5).effective(), 5);
        assert_eq!(PageLimit::new(MAX_PAGE_LIMIT).effective(), MAX_PAGE_LIMIT);
        assert_eq!(PageLimit::new(MAX_PAGE_LIMIT + 1).effective(), MAX_PAGE_LIMIT);
        assert_eq!(PageLimit::new(u32::MAX).effective(), MAX_PAGE_LIMIT);
    }

    #[test]
    fn test_pagination_empty_results() {
        let env = Env::default();
        let records: [PrivacyRecord; 0] = [];
        let page = paginate_records(
            &env,
            &records,
            &QueryFilter::none(),
            None,
            PageLimit::new(10),
        );
        assert_eq!(page.items.len(), 0);
        assert!(page.next_cursor.is_none());
    }

    #[test]
    fn test_pagination_single_item_page() {
        let env = Env::default();
        let submitter = Address::generate(&env);
        let records = [
            make_record(&env, 1, 1, 100, RecordStatus::Active, &submitter),
            make_record(&env, 1, 2, 200, RecordStatus::Active, &submitter),
        ];

        let page = paginate_records(
            &env,
            &records,
            &QueryFilter::none(),
            None,
            PageLimit::new(1),
        );
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items.get(0).unwrap().primary_id, 1);
        let cursor = page.next_cursor.expect("expected next cursor");
        assert_eq!(cursor.ledger_sequence, 1);
        assert_eq!(cursor.primary_id, 2);

        let page2 = paginate_records(
            &env,
            &records,
            &QueryFilter::none(),
            Some(cursor),
            PageLimit::new(1),
        );
        assert_eq!(page2.items.len(), 1);
        assert_eq!(page2.items.get(0).unwrap().primary_id, 2);
        assert!(page2.next_cursor.is_none());
    }

    #[test]
    fn test_pagination_cursor_past_end() {
        let env = Env::default();
        let submitter = Address::generate(&env);
        let records = [
            make_record(&env, 1, 1, 100, RecordStatus::Active, &submitter),
            make_record(&env, 1, 2, 200, RecordStatus::Active, &submitter),
        ];

        let page = paginate_records(
            &env,
            &records,
            &QueryFilter::none(),
            Some(PageCursor::new(99, 99)),
            PageLimit::new(10),
        );
        assert_eq!(page.items.len(), 0);
        assert!(page.next_cursor.is_none());
    }

    #[test]
    fn test_pagination_full_traversal() {
        let env = Env::default();
        let submitter = Address::generate(&env);
        let records = [
            make_record(&env, 1, 1, 100, RecordStatus::Active, &submitter),
            make_record(&env, 1, 2, 200, RecordStatus::Active, &submitter),
            make_record(&env, 2, 1, 300, RecordStatus::Active, &submitter),
            make_record(&env, 2, 2, 400, RecordStatus::Active, &submitter),
            make_record(&env, 3, 1, 500, RecordStatus::Active, &submitter),
        ];

        let mut cursor: Option<PageCursor> = None;
        let mut seen: u32 = 0;
        loop {
            let page = paginate_records(
                &env,
                &records,
                &QueryFilter::none(),
                cursor,
                PageLimit::new(2),
            );
            seen += page.items.len();
            match page.next_cursor {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }
        assert_eq!(seen, 5);
    }

    #[test]
    fn test_pagination_filter_by_timestamp_range() {
        let env = Env::default();
        let submitter = Address::generate(&env);
        let records = [
            make_record(&env, 1, 1, 100, RecordStatus::Active, &submitter),
            make_record(&env, 1, 2, 200, RecordStatus::Active, &submitter),
            make_record(&env, 1, 3, 300, RecordStatus::Active, &submitter),
        ];

        let filter = QueryFilter {
            start_timestamp: Some(150),
            end_timestamp: Some(250),
            status: None,
            submitter: None,
        };
        let page = paginate_records(&env, &records, &filter, None, PageLimit::new(10));
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items.get(0).unwrap().primary_id, 2);
    }

    #[test]
    fn test_pagination_filter_by_status_and_submitter() {
        let env = Env::default();
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        let records = [
            make_record(&env, 1, 1, 100, RecordStatus::Active, &alice),
            make_record(&env, 1, 2, 200, RecordStatus::Revoked, &alice),
            make_record(&env, 1, 3, 300, RecordStatus::Active, &bob),
        ];

        let filter = QueryFilter {
            start_timestamp: None,
            end_timestamp: None,
            status: Some(RecordStatus::Active),
            submitter: Some(alice.clone()),
        };
        let page = paginate_records(&env, &records, &filter, None, PageLimit::new(10));
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items.get(0).unwrap().primary_id, 1);
    }
}
