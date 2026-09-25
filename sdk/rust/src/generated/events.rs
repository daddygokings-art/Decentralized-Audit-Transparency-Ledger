// GENERATED FILE — DO NOT EDIT.
// Regenerate with: npm run generate --workspace tools/sdk-codegen (or `sdk-codegen generate`).
// Source of truth: abi/audit-ledger.json
//
// Typed contract events. In Soroban the trailing topic is the event
// discriminator; `topics` below lists the discriminator-relative topics.

/// Event name constants, exactly as published on chain.
pub mod event_name {
    /// `allowlist_disabled`
    pub const ALLOWLIST_DISABLED: &str = "allowlist_disabled";
    /// `allowlist_enabled`
    pub const ALLOWLIST_ENABLED: &str = "allowlist_enabled";
    /// `archived_events_purged`
    pub const ARCHIVED_EVENTS_PURGED: &str = "archived_events_purged";
    /// `config_set`
    pub const CONFIG_SET: &str = "config_set";
    /// `contract_paused`
    pub const CONTRACT_PAUSED: &str = "contract_paused";
    /// `contract_unpaused`
    pub const CONTRACT_UNPAUSED: &str = "contract_unpaused";
    /// `contract_upgraded`
    pub const CONTRACT_UPGRADED: &str = "contract_upgraded";
    /// `default_nonce_config_set`
    pub const DEFAULT_NONCE_CONFIG_SET: &str = "default_nonce_config_set";
    /// `event_rolled_back`
    pub const EVENT_ROLLED_BACK: &str = "event_rolled_back";
    /// `event_updated`
    pub const EVENT_UPDATED: &str = "event_updated";
    /// `events_archived`
    pub const EVENTS_ARCHIVED: &str = "events_archived";
    /// `expired_removed`
    pub const EXPIRED_REMOVED: &str = "expired_removed";
    /// `log_event`
    pub const LOG_EVENT: &str = "log_event";
    /// `migrate_event_metadata`
    pub const MIGRATE_EVENT_METADATA: &str = "migrate_event_metadata";
    /// `nonce_config_set`
    pub const NONCE_CONFIG_SET: &str = "nonce_config_set";
    /// `nonce_reset`
    pub const NONCE_RESET: &str = "nonce_reset";
    /// `owner_added`
    pub const OWNER_ADDED: &str = "owner_added";
    /// `owner_removed`
    pub const OWNER_REMOVED: &str = "owner_removed";
    /// `policy_set`
    pub const POLICY_SET: &str = "policy_set";
    /// `policy_set_type`
    pub const POLICY_SET_TYPE: &str = "policy_set_type";
    /// `proposal_approved`
    pub const PROPOSAL_APPROVED: &str = "proposal_approved";
    /// `proposal_executed`
    pub const PROPOSAL_EXECUTED: &str = "proposal_executed";
    /// `proposal_submitted`
    pub const PROPOSAL_SUBMITTED: &str = "proposal_submitted";
    /// `rbac_toggled`
    pub const RBAC_TOGGLED: &str = "rbac_toggled";
    /// `register_schema`
    pub const REGISTER_SCHEMA: &str = "register_schema";
    /// `register_webhook`
    pub const REGISTER_WEBHOOK: &str = "register_webhook";
    /// `remove_event_cap`
    pub const REMOVE_EVENT_CAP: &str = "remove_event_cap";
    /// `required_signatures_set`
    pub const REQUIRED_SIGNATURES_SET: &str = "required_signatures_set";
    /// `role_set`
    pub const ROLE_SET: &str = "role_set";
    /// `set_event_ttl`
    pub const SET_EVENT_TTL: &str = "set_event_ttl";
    /// `set_global_max`
    pub const SET_GLOBAL_MAX: &str = "set_global_max";
    /// `set_metadata_schema`
    pub const SET_METADATA_SCHEMA: &str = "set_metadata_schema";
    /// `snapshot_created`
    pub const SNAPSHOT_CREATED: &str = "snapshot_created";
    /// `stale_dedup_cleaned`
    pub const STALE_DEDUP_CLEANED: &str = "stale_dedup_cleaned";
    /// `stale_hashes_cleaned`
    pub const STALE_HASHES_CLEANED: &str = "stale_hashes_cleaned";
    /// `storage_compacted`
    pub const STORAGE_COMPACTED: &str = "storage_compacted";
    /// `submitter_allowed`
    pub const SUBMITTER_ALLOWED: &str = "submitter_allowed";
    /// `submitter_blocked`
    pub const SUBMITTER_BLOCKED: &str = "submitter_blocked";
    /// `submitter_removed_from_allowlist`
    pub const SUBMITTER_REMOVED_FROM_ALLOWLIST: &str = "submitter_removed_from_allowlist";
    /// `submitter_unblocked`
    pub const SUBMITTER_UNBLOCKED: &str = "submitter_unblocked";
    /// `transfer_ownership`
    pub const TRANSFER_OWNERSHIP: &str = "transfer_ownership";
    /// `unregister_webhook`
    pub const UNREGISTER_WEBHOOK: &str = "unregister_webhook";
    /// `version_tagged`
    pub const VERSION_TAGGED: &str = "version_tagged";
}

/// All event names known to this SDK, in alphabetical order.
pub const ALL_EVENT_NAMES: [&str; 43] = [
    "allowlist_disabled",
    "allowlist_enabled",
    "archived_events_purged",
    "config_set",
    "contract_paused",
    "contract_unpaused",
    "contract_upgraded",
    "default_nonce_config_set",
    "event_rolled_back",
    "event_updated",
    "events_archived",
    "expired_removed",
    "log_event",
    "migrate_event_metadata",
    "nonce_config_set",
    "nonce_reset",
    "owner_added",
    "owner_removed",
    "policy_set",
    "policy_set_type",
    "proposal_approved",
    "proposal_executed",
    "proposal_submitted",
    "rbac_toggled",
    "register_schema",
    "register_webhook",
    "remove_event_cap",
    "required_signatures_set",
    "role_set",
    "set_event_ttl",
    "set_global_max",
    "set_metadata_schema",
    "snapshot_created",
    "stale_dedup_cleaned",
    "stale_hashes_cleaned",
    "storage_compacted",
    "submitter_allowed",
    "submitter_blocked",
    "submitter_removed_from_allowlist",
    "submitter_unblocked",
    "transfer_ownership",
    "unregister_webhook",
    "version_tagged",
];

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct AllowlistDisabledEvent {
    /// Data 1: address
    pub data1: Address,
}

impl AllowlistDisabledEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "allowlist_disabled";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct AllowlistEnabledEvent {
    /// Data 1: address
    pub data1: Address,
}

impl AllowlistEnabledEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "allowlist_enabled";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct ArchivedEventsPurgedEvent {
    /// Data 1: u32
    pub data1: u32,
}

impl ArchivedEventsPurgedEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "archived_events_purged";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigSetEvent {
    /// Topic 1: symbol
    pub topic1: Symbol,
    /// Data 1: address
    /// Data 2: bool
    /// Data 3: u32
    pub data1: Address,
    pub data2: bool,
    pub data3: u32,
}

impl ConfigSetEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "config_set";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct ContractPausedEvent {
    /// Data 1: address
    pub data1: Address,
}

impl ContractPausedEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "contract_paused";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct ContractUnpausedEvent {
    /// Data 1: address
    pub data1: Address,
}

impl ContractUnpausedEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "contract_unpaused";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct ContractUpgradedEvent {
    /// Data 1: option<bytesn<32>>
    /// Data 2: bytesn<32>
    pub data1: Option<BytesN<32>>,
    pub data2: BytesN<32>,
}

impl ContractUpgradedEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "contract_upgraded";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct DefaultNonceConfigSetEvent {
    /// Data 1: address
    /// Data 2: u32
    /// Data 3: u32
    pub data1: Address,
    pub data2: u32,
    pub data3: u32,
}

impl DefaultNonceConfigSetEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "default_nonce_config_set";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct EventRolledBackEvent {
    /// Topic 1: symbol
    pub topic1: Symbol,
    /// Data 1: u32
    /// Data 2: u32
    /// Data 3: address
    /// Data 4: u64
    pub data1: u32,
    pub data2: u32,
    pub data3: Address,
    pub data4: u64,
}

impl EventRolledBackEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "event_rolled_back";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct EventUpdatedEvent {
    /// Data 1: u32
    /// Data 2: bytesn<32>
    /// Data 3: bytesn<32>
    /// Data 4: address
    /// Data 5: u64
    pub data1: u32,
    pub data2: BytesN<32>,
    pub data3: BytesN<32>,
    pub data4: Address,
    pub data5: u64,
}

impl EventUpdatedEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "event_updated";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct EventsArchivedEvent {
    /// Data 1: u32
    pub data1: u32,
}

impl EventsArchivedEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "events_archived";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct ExpiredRemovedEvent {
    /// Topic 1: symbol
    pub topic1: Symbol,
    /// Data 1: address
    /// Data 2: u32
    /// Data 3: u32
    /// Data 4: u32
    pub data1: Address,
    pub data2: u32,
    pub data3: u32,
    pub data4: u32,
}

impl ExpiredRemovedEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "expired_removed";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct LogEventEvent {
    /// Topic 1: symbol
    pub topic1: Symbol,
    /// Data 1: address
    /// Data 2: symbol
    /// Data 3: u32
    pub data1: Address,
    pub data2: Symbol,
    pub data3: u32,
}

impl LogEventEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "log_event";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct MigrateEventMetadataEvent {
    /// Topic 1: symbol
    pub topic1: Symbol,
    /// Data 1: address
    /// Data 2: symbol
    /// Data 3: u32
    /// Data 4: u32
    pub data1: Address,
    pub data2: Symbol,
    pub data3: u32,
    pub data4: u32,
}

impl MigrateEventMetadataEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "migrate_event_metadata";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct NonceConfigSetEvent {
    /// Data 1: address
    /// Data 2: u32
    /// Data 3: u32
    pub data1: Address,
    pub data2: u32,
    pub data3: u32,
}

impl NonceConfigSetEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "nonce_config_set";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct NonceResetEvent {
    /// Data 1: address
    /// Data 2: address
    pub data1: Address,
    pub data2: Address,
}

impl NonceResetEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "nonce_reset";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct OwnerAddedEvent {
    /// Data 1: address
    pub data1: Address,
}

impl OwnerAddedEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "owner_added";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct OwnerRemovedEvent {
    /// Data 1: address
    pub data1: Address,
}

impl OwnerRemovedEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "owner_removed";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct PolicySetEvent {
    /// Topic 1: symbol
    pub topic1: Symbol,
    /// Data 1: address
    /// Data 2: u32
    pub data1: Address,
    pub data2: u32,
}

impl PolicySetEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "policy_set";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct PolicySetTypeEvent {
    /// Topic 1: symbol
    pub topic1: Symbol,
    /// Data 1: address
    /// Data 2: symbol
    /// Data 3: option<u32>
    pub data1: Address,
    pub data2: Symbol,
    pub data3: Option<u32>,
}

impl PolicySetTypeEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "policy_set_type";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct ProposalApprovedEvent {
    /// Data 1: u32
    /// Data 2: address
    pub data1: u32,
    pub data2: Address,
}

impl ProposalApprovedEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "proposal_approved";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct ProposalExecutedEvent {
    /// Data 1: u32
    /// Data 2: address
    pub data1: u32,
    pub data2: Address,
}

impl ProposalExecutedEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "proposal_executed";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct ProposalSubmittedEvent {
    /// Data 1: bytesn<32>
    pub data1: BytesN<32>,
}

impl ProposalSubmittedEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "proposal_submitted";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct RbacToggledEvent {
    /// Topic 1: symbol
    pub topic1: Symbol,
    /// Data 1: address
    /// Data 2: bool
    pub data1: Address,
    pub data2: bool,
}

impl RbacToggledEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "rbac_toggled";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct RegisterSchemaEvent {
    /// Topic 1: symbol
    pub topic1: Symbol,
    /// Data 1: address
    /// Data 2: symbol
    /// Data 3: u32
    pub data1: Address,
    pub data2: Symbol,
    pub data3: u32,
}

impl RegisterSchemaEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "register_schema";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct RegisterWebhookEvent {
    /// Topic 1: symbol
    pub topic1: Symbol,
    /// Data 1: address
    /// Data 2: symbol
    /// Data 3: bytes
    pub data1: Address,
    pub data2: Symbol,
    pub data3: Bytes,
}

impl RegisterWebhookEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "register_webhook";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct RemoveEventCapEvent {
    /// Topic 1: symbol
    pub topic1: Symbol,
    /// Data 1: address
    /// Data 2: symbol
    pub data1: Address,
    pub data2: Symbol,
}

impl RemoveEventCapEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "remove_event_cap";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct RequiredSignaturesSetEvent {
    /// Data 1: u32
    pub data1: u32,
}

impl RequiredSignaturesSetEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "required_signatures_set";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct RoleSetEvent {
    /// Topic 1: symbol
    pub topic1: Symbol,
    /// Data 1: address
    /// Data 2: address
    /// Data 3: option<Role>
    pub data1: Address,
    pub data2: Address,
    pub data3: Option<super::Role>,
}

impl RoleSetEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "role_set";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct SetEventTtlEvent {
    /// Topic 1: symbol
    pub topic1: Symbol,
    /// Data 1: address
    /// Data 2: u32
    /// Data 3: u32
    pub data1: Address,
    pub data2: u32,
    pub data3: u32,
}

impl SetEventTtlEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "set_event_ttl";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct SetGlobalMaxEvent {
    /// Topic 1: symbol
    pub topic1: Symbol,
    /// Data 1: address
    /// Data 2: u32
    /// Data 3: u32
    pub data1: Address,
    pub data2: u32,
    pub data3: u32,
}

impl SetGlobalMaxEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "set_global_max";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct SetMetadataSchemaEvent {
    /// Topic 1: symbol
    pub topic1: Symbol,
    /// Data 1: address
    /// Data 2: symbol
    /// Data 3: u32
    pub data1: Address,
    pub data2: Symbol,
    pub data3: u32,
}

impl SetMetadataSchemaEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "set_metadata_schema";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct SnapshotCreatedEvent {
    /// Data 1: u32
    /// Data 2: u64
    /// Data 3: u32
    pub data1: u32,
    pub data2: u64,
    pub data3: u32,
}

impl SnapshotCreatedEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "snapshot_created";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct StaleDedupCleanedEvent {
    /// Data 1: u32
    pub data1: u32,
}

impl StaleDedupCleanedEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "stale_dedup_cleaned";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct StaleHashesCleanedEvent {
    /// Data 1: u32
    pub data1: u32,
}

impl StaleHashesCleanedEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "stale_hashes_cleaned";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct StorageCompactedEvent {
    /// Data 1: u32
    pub data1: u32,
}

impl StorageCompactedEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "storage_compacted";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct SubmitterAllowedEvent {
    /// Data 1: address
    /// Data 2: address
    pub data1: Address,
    pub data2: Address,
}

impl SubmitterAllowedEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "submitter_allowed";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct SubmitterBlockedEvent {
    /// Data 1: address
    /// Data 2: address
    pub data1: Address,
    pub data2: Address,
}

impl SubmitterBlockedEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "submitter_blocked";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct SubmitterRemovedFromAllowlistEvent {
    /// Data 1: address
    /// Data 2: address
    pub data1: Address,
    pub data2: Address,
}

impl SubmitterRemovedFromAllowlistEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "submitter_removed_from_allowlist";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct SubmitterUnblockedEvent {
    /// Data 1: address
    /// Data 2: address
    pub data1: Address,
    pub data2: Address,
}

impl SubmitterUnblockedEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "submitter_unblocked";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct TransferOwnershipEvent {
    /// Topic 1: symbol
    pub topic1: Symbol,
    /// Data 1: address
    /// Data 2: address
    /// Data 3: address
    pub data1: Address,
    pub data2: Address,
    pub data3: Address,
}

impl TransferOwnershipEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "transfer_ownership";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct UnregisterWebhookEvent {
    /// Topic 1: symbol
    pub topic1: Symbol,
    /// Data 1: address
    /// Data 2: symbol
    /// Data 3: bytes
    pub data1: Address,
    pub data2: Symbol,
    pub data3: Bytes,
}

impl UnregisterWebhookEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "unregister_webhook";
}

/// Decoded contract event payload.
#[derive(Clone, Debug, PartialEq)]
pub struct VersionTaggedEvent {
    /// Topic 1: symbol
    pub topic1: Symbol,
    /// Data 1: address
    /// Data 2: u32
    /// Data 3: u32
    /// Data 4: symbol
    pub data1: Address,
    pub data2: u32,
    pub data3: u32,
    pub data4: Symbol,
}

impl VersionTaggedEvent {
    /// The on-chain event name for this payload.
    pub const NAME: &'static str = "version_tagged";
}

// contract: AuditLedger v0.1.0
// idl: spec_version 1.0.0 (120 functions, 19 types, 75 errors, 43 events)
