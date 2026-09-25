// GENERATED FILE — DO NOT EDIT.
// Regenerate with: npm run generate --workspace tools/sdk-codegen (or `sdk-codegen generate`).
// Source of truth: abi/audit-ledger.json
//
// Type-safe client for the AuditLedger Soroban contract.
//
// The client is transport-agnostic: supply any implementation of
// `ContractTransport`. Argument and return types come straight from the
// contract IDL, so an unregenerated contract change fails to compile.

use super::types::*;

/// Transport contract: invokes a contract function and returns its result.
pub trait ContractTransport {
    /// Invoke `method` (the on-chain function name) with positional `args`.
    fn invoke(&self, method: &str, args: Vec<Val>) -> Result<Val, TransportError>;
}

/// Opaque Soroban value passed across the transport boundary.
pub type Val = soroban_sdk::Val;

/// Error returned by a [`ContractTransport`] implementation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransportError {
    /// The transport could not reach the RPC endpoint.
    Rpc(String),
    /// The contract reverted; `code` is the on-chain error code.
    Contract(u32),
    /// The SDK and the deployed contract disagree about the surface.
    Decode(String),
}

/// Generated client for AuditLedger.
pub struct GeneratedClient<T: ContractTransport> {
    contract_id: Address,
    transport: T,
}

impl<T: ContractTransport> GeneratedClient<T> {
    /// Bind a transport to a contract address.
    pub fn new(contract_id: Address, transport: T) -> Self {
        Self { contract_id, transport }
    }

    /// The contract address this client invokes.
    pub fn contract_id(&self) -> &Address {
        &self.contract_id
    }

    /// All contract function names this client exposes.
    pub const FUNCTION_NAMES: [&'static str; 120] = [
        "initialize",
        "log_events",
        "log_event",
        "log_event_with_hierarchy",
        "log_event_with_nonce",
        "get_submitter_nonce",
        "get_submitter_nonce_state",
        "set_submitter_nonce_config",
        "reset_submitter_nonce",
        "set_default_nonce_config",
        "get_default_nonce_window_size",
        "get_default_nonce_max_value",
        "total_events",
        "get_event_type_count",
        "get_event",
        "get_event_metadata",
        "get_event_header",
        "get_event_by_order",
        "event_count",
        "event_count_by_category",
        "list_events_by_category",
        "archive_events",
        "get_archived_event",
        "get_archived_event_ref",
        "verify_archived_event_checksum",
        "get_archive_stats",
        "get_archived_event_count",
        "list_archived_events",
        "purge_archived_events",
        "upgrade_contract",
        "get_event_by_type",
        "list_events",
        "list_events_by_type",
        "get_events_by_type",
        "submitter_event_count",
        "get_event_by_submitter",
        "get_events_by_submitter",
        "get_events_by_time_range",
        "search_events",
        "update_event",
        "get_event_history",
        "rollback_event",
        "get_event_version_count",
        "compare_event_versions",
        "verify_integrity",
        "verify_integrity_range",
        "create_snapshot",
        "get_snapshot",
        "snapshot_count",
        "verify_snapshot",
        "cleanup_stale_hashes",
        "set_global_max_logs",
        "set_event_max_logs",
        "remove_event_cap",
        "has_cap",
        "transfer_ownership",
        "set_metadata_max_size",
        "set_event_metadata_max_size",
        "set_metadata_schema",
        "get_metadata_schema",
        "register_schema",
        "get_schema",
        "list_schemas",
        "migrate_event_metadata",
        "get_migration_function",
        "check_schema_compatibility",
        "is_backward_compatible",
        "is_forward_compatible",
        "set_event_ttl",
        "get_event_ttl",
        "cleanup_expired_events",
        "get_cleanup_stats",
        "register_webhook",
        "unregister_webhook",
        "get_webhooks",
        "pause",
        "unpause",
        "is_paused",
        "paused_since",
        "set_category_max_len",
        "block_submitter",
        "unblock_submitter",
        "enable_allowlist_mode",
        "disable_allowlist_mode",
        "allow_submitter",
        "remove_submitter_from_allowlist",
        "get_metadata_max_size",
        "get_statistics",
        "set_event_emission_mode",
        "get_event_emission_mode",
        "set_low_cost_mode",
        "is_low_cost_mode",
        "set_submitter_rate_limit",
        "compact_storage",
        "log_event_signed",
        "get_event_signature",
        "find_event_by_content",
        "add_owner",
        "remove_owner",
        "set_required_signatures",
        "submit_proposal",
        "approve_proposal",
        "execute_proposal",
        "set_role",
        "get_role",
        "enable_rbac",
        "is_rbac_enabled",
        "set_dedup_policy",
        "get_dedup_policy",
        "set_dedup_policy_for_type",
        "get_dedup_policy_for_type",
        "log_event_with_custom_key",
        "cleanup_stale_dedup_entries",
        "set_archive_config",
        "get_archive_config",
        "get_event_audit_trail",
        "tag_event_version",
        "get_event_version_tag",
        "get_event_diff",
        "compare_event_versions_detailed",
    ];

    #[doc = "State-changing contract function `initialize`."]
    #[doc = "Wire name `owners` (vec<address>)."]
    #[doc = "Wire name `global_max_logs` (u32)."]
    #[doc = "Wire name `max_metadata_bytes` (u32)."]
    pub fn initialize(
        &self,
        owners: Vec<Address>,
        global_max_logs: u32,
        max_metadata_bytes: u32,
    ) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(owners),
            Into::<Val>::into(global_max_logs),
            Into::<Val>::into(max_metadata_bytes),
        ];
        self.transport.invoke("initialize", args)
    }

    #[doc = "State-changing contract function `log_events`."]
    #[doc = "Wire name `events` (vec<tuple<address,symbol,bytes>>)."]
    pub fn log_events(&self, events: Vec<(Address, Symbol, Bytes)>) -> Result<Vec<u32>, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(events)];
        self.transport.invoke("log_events", args)
    }

    #[doc = "State-changing contract function `log_event`."]
    #[doc = "Wire name `submitter` (address)."]
    #[doc = "Wire name `event_type` (symbol)."]
    #[doc = "Wire name `metadata` (bytes)."]
    #[doc = "Wire name `category` (option<symbol>)."]
    #[doc = "Wire name `sub_event_type` (option<symbol>)."]
    #[doc = "Wire name `force` (bool)."]
    pub fn log_event(
        &self,
        submitter: Address,
        event_type: Symbol,
        metadata: Bytes,
        category: Option<Symbol>,
        sub_event_type: Option<Symbol>,
        force: bool,
    ) -> Result<BytesN<32>, TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(submitter),
            Into::<Val>::into(event_type),
            Into::<Val>::into(metadata),
            Into::<Val>::into(category),
            Into::<Val>::into(sub_event_type),
            Into::<Val>::into(force),
        ];
        self.transport.invoke("log_event", args)
    }

    #[doc = "State-changing contract function `log_event_with_hierarchy`."]
    #[doc = "Wire name `submitter` (address)."]
    #[doc = "Wire name `event_type` (symbol)."]
    #[doc = "Wire name `metadata` (bytes)."]
    #[doc = "Wire name `category` (option<symbol>)."]
    #[doc = "Wire name `sub_event_type` (option<symbol>)."]
    #[doc = "Wire name `force` (bool)."]
    pub fn log_event_with_hierarchy(
        &self,
        submitter: Address,
        event_type: Symbol,
        metadata: Bytes,
        category: Option<Symbol>,
        sub_event_type: Option<Symbol>,
        force: bool,
    ) -> Result<BytesN<32>, TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(submitter),
            Into::<Val>::into(event_type),
            Into::<Val>::into(metadata),
            Into::<Val>::into(category),
            Into::<Val>::into(sub_event_type),
            Into::<Val>::into(force),
        ];
        self.transport.invoke("log_event_with_hierarchy", args)
    }

    #[doc = "State-changing contract function `log_event_with_nonce`."]
    #[doc = "Wire name `submitter` (address)."]
    #[doc = "Wire name `event_type` (symbol)."]
    #[doc = "Wire name `metadata` (bytes)."]
    #[doc = "Wire name `nonce` (u32)."]
    pub fn log_event_with_nonce(
        &self,
        submitter: Address,
        event_type: Symbol,
        metadata: Bytes,
        nonce: u32,
    ) -> Result<BytesN<32>, TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(submitter),
            Into::<Val>::into(event_type),
            Into::<Val>::into(metadata),
            Into::<Val>::into(nonce),
        ];
        self.transport.invoke("log_event_with_nonce", args)
    }

    #[doc = "Read-only contract function `get_submitter_nonce`."]
    #[doc = "Wire name `submitter` (address)."]
    pub fn get_submitter_nonce(&self, submitter: Address) -> Result<u32, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(submitter)];
        self.transport.invoke("get_submitter_nonce", args)
    }

    #[doc = "Read-only contract function `get_submitter_nonce_state`."]
    #[doc = "Wire name `submitter` (address)."]
    pub fn get_submitter_nonce_state(&self, submitter: Address) -> Result<super::NonceState, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(submitter)];
        self.transport.invoke("get_submitter_nonce_state", args)
    }

    #[doc = "State-changing contract function `set_submitter_nonce_config`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `submitter` (address)."]
    #[doc = "Wire name `window_size` (u32)."]
    #[doc = "Wire name `max_nonce` (u32)."]
    pub fn set_submitter_nonce_config(
        &self,
        caller: Address,
        submitter: Address,
        window_size: u32,
        max_nonce: u32,
    ) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(caller),
            Into::<Val>::into(submitter),
            Into::<Val>::into(window_size),
            Into::<Val>::into(max_nonce),
        ];
        self.transport.invoke("set_submitter_nonce_config", args)
    }

    #[doc = "State-changing contract function `reset_submitter_nonce`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `submitter` (address)."]
    pub fn reset_submitter_nonce(&self, caller: Address, submitter: Address) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(caller), Into::<Val>::into(submitter)];
        self.transport.invoke("reset_submitter_nonce", args)
    }

    #[doc = "State-changing contract function `set_default_nonce_config`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `window_size` (u32)."]
    #[doc = "Wire name `max_nonce` (u32)."]
    pub fn set_default_nonce_config(
        &self,
        caller: Address,
        window_size: u32,
        max_nonce: u32,
    ) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(caller),
            Into::<Val>::into(window_size),
            Into::<Val>::into(max_nonce),
        ];
        self.transport.invoke("set_default_nonce_config", args)
    }

    #[doc = "Read-only contract function `get_default_nonce_window_size`."]
    pub fn get_default_nonce_window_size(&self) -> Result<u32, TransportError> {
        let args: Vec<Val> = vec![];
        self.transport.invoke("get_default_nonce_window_size", args)
    }

    #[doc = "Read-only contract function `get_default_nonce_max_value`."]
    pub fn get_default_nonce_max_value(&self) -> Result<u32, TransportError> {
        let args: Vec<Val> = vec![];
        self.transport.invoke("get_default_nonce_max_value", args)
    }

    #[doc = "Read-only contract function `total_events`."]
    pub fn total_events(&self) -> Result<u32, TransportError> {
        let args: Vec<Val> = vec![];
        self.transport.invoke("total_events", args)
    }

    #[doc = "Read-only contract function `get_event_type_count`."]
    #[doc = "Wire name `event_type` (symbol)."]
    pub fn get_event_type_count(&self, event_type: Symbol) -> Result<u32, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(event_type)];
        self.transport.invoke("get_event_type_count", args)
    }

    #[doc = "Read-only contract function `get_event`."]
    #[doc = "Wire name `id` (bytesn<32>)."]
    pub fn get_event(&self, id: BytesN<32>) -> Result<super::Event, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(id)];
        self.transport.invoke("get_event", args)
    }

    #[doc = "Read-only contract function `get_event_metadata`."]
    #[doc = "Wire name `id` (bytesn<32>)."]
    pub fn get_event_metadata(&self, id: BytesN<32>) -> Result<Bytes, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(id)];
        self.transport.invoke("get_event_metadata", args)
    }

    #[doc = "Read-only contract function `get_event_header`."]
    #[doc = "Wire name `id` (bytesn<32>)."]
    pub fn get_event_header(&self, id: BytesN<32>) -> Result<super::EventHeader, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(id)];
        self.transport.invoke("get_event_header", args)
    }

    #[doc = "Read-only contract function `get_event_by_order`."]
    #[doc = "Wire name `order` (u32)."]
    pub fn get_event_by_order(&self, order: u32) -> Result<super::Event, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(order)];
        self.transport.invoke("get_event_by_order", args)
    }

    #[doc = "Read-only contract function `event_count`."]
    #[doc = "Wire name `event_type` (symbol)."]
    pub fn event_count(&self, event_type: Symbol) -> Result<u32, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(event_type)];
        self.transport.invoke("event_count", args)
    }

    #[doc = "Read-only contract function `event_count_by_category`."]
    #[doc = "Wire name `category` (symbol)."]
    pub fn event_count_by_category(&self, category: Symbol) -> Result<u32, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(category)];
        self.transport.invoke("event_count_by_category", args)
    }

    #[doc = "Read-only contract function `list_events_by_category`."]
    #[doc = "Wire name `category` (symbol)."]
    #[doc = "Wire name `start` (u32)."]
    #[doc = "Wire name `limit` (u32)."]
    pub fn list_events_by_category(
        &self,
        category: Symbol,
        start: u32,
        limit: u32,
    ) -> Result<Vec<super::EventHeader>, TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(category),
            Into::<Val>::into(start),
            Into::<Val>::into(limit),
        ];
        self.transport.invoke("list_events_by_category", args)
    }

    #[doc = "State-changing contract function `archive_events`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `cutoff_timestamp` (u64)."]
    pub fn archive_events(&self, caller: Address, cutoff_timestamp: u64) -> Result<u32, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(caller), Into::<Val>::into(cutoff_timestamp)];
        self.transport.invoke("archive_events", args)
    }

    #[doc = "Read-only contract function `get_archived_event`."]
    #[doc = "Wire name `id` (bytesn<32>)."]
    pub fn get_archived_event(&self, id: BytesN<32>) -> Result<super::Event, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(id)];
        self.transport.invoke("get_archived_event", args)
    }

    #[doc = "Read-only contract function `get_archived_event_ref`."]
    #[doc = "Wire name `id` (bytesn<32>)."]
    pub fn get_archived_event_ref(&self, id: BytesN<32>) -> Result<Option<super::ArchivedEventRef>, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(id)];
        self.transport.invoke("get_archived_event_ref", args)
    }

    #[doc = "Read-only contract function `verify_archived_event_checksum`."]
    #[doc = "Wire name `id` (bytesn<32>)."]
    #[doc = "Wire name `candidate` (Event)."]
    pub fn verify_archived_event_checksum(
        &self,
        id: BytesN<32>,
        candidate: super::Event,
    ) -> Result<bool, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(id), Into::<Val>::into(candidate)];
        self.transport.invoke("verify_archived_event_checksum", args)
    }

    #[doc = "Read-only contract function `get_archive_stats`."]
    pub fn get_archive_stats(&self) -> Result<super::ArchiveStats, TransportError> {
        let args: Vec<Val> = vec![];
        self.transport.invoke("get_archive_stats", args)
    }

    #[doc = "Read-only contract function `get_archived_event_count`."]
    pub fn get_archived_event_count(&self) -> Result<u32, TransportError> {
        let args: Vec<Val> = vec![];
        self.transport.invoke("get_archived_event_count", args)
    }

    #[doc = "Read-only contract function `list_archived_events`."]
    #[doc = "Wire name `start` (u32)."]
    #[doc = "Wire name `limit` (u32)."]
    pub fn list_archived_events(&self, start: u32, limit: u32) -> Result<Vec<super::EventHeader>, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(start), Into::<Val>::into(limit)];
        self.transport.invoke("list_archived_events", args)
    }

    #[doc = "State-changing contract function `purge_archived_events`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `cutoff_timestamp` (u64)."]
    #[doc = "Wire name `confirm` (bool)."]
    pub fn purge_archived_events(
        &self,
        caller: Address,
        cutoff_timestamp: u64,
        confirm: bool,
    ) -> Result<u32, TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(caller),
            Into::<Val>::into(cutoff_timestamp),
            Into::<Val>::into(confirm),
        ];
        self.transport.invoke("purge_archived_events", args)
    }

    #[doc = "State-changing contract function `upgrade_contract`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `new_wasm_hash` (bytesn<32>)."]
    pub fn upgrade_contract(&self, caller: Address, new_wasm_hash: BytesN<32>) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(caller), Into::<Val>::into(new_wasm_hash)];
        self.transport.invoke("upgrade_contract", args)
    }

    #[doc = "Read-only contract function `get_event_by_type`."]
    #[doc = "Wire name `event_type` (symbol)."]
    #[doc = "Wire name `type_index` (u32)."]
    pub fn get_event_by_type(&self, event_type: Symbol, type_index: u32) -> Result<super::Event, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(event_type), Into::<Val>::into(type_index)];
        self.transport.invoke("get_event_by_type", args)
    }

    #[doc = "Read-only contract function `list_events`."]
    #[doc = "Wire name `offset` (u32)."]
    #[doc = "Wire name `limit` (u32)."]
    pub fn list_events(&self, offset: u32, limit: u32) -> Result<Vec<super::Event>, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(offset), Into::<Val>::into(limit)];
        self.transport.invoke("list_events", args)
    }

    #[doc = "Read-only contract function `list_events_by_type`."]
    #[doc = "Wire name `event_type` (symbol)."]
    #[doc = "Wire name `offset` (u32)."]
    #[doc = "Wire name `limit` (u32)."]
    pub fn list_events_by_type(
        &self,
        event_type: Symbol,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<super::Event>, TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(event_type),
            Into::<Val>::into(offset),
            Into::<Val>::into(limit),
        ];
        self.transport.invoke("list_events_by_type", args)
    }

    #[doc = "Read-only contract function `get_events_by_type`."]
    #[doc = "Wire name `event_type` (symbol)."]
    #[doc = "Wire name `start` (u32)."]
    #[doc = "Wire name `limit` (u32)."]
    pub fn get_events_by_type(
        &self,
        event_type: Symbol,
        start: u32,
        limit: u32,
    ) -> Result<Vec<super::Event>, TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(event_type),
            Into::<Val>::into(start),
            Into::<Val>::into(limit),
        ];
        self.transport.invoke("get_events_by_type", args)
    }

    #[doc = "Read-only contract function `submitter_event_count`."]
    #[doc = "Wire name `submitter` (address)."]
    pub fn submitter_event_count(&self, submitter: Address) -> Result<u32, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(submitter)];
        self.transport.invoke("submitter_event_count", args)
    }

    #[doc = "Read-only contract function `get_event_by_submitter`."]
    #[doc = "Wire name `submitter` (address)."]
    #[doc = "Wire name `submitter_index` (u32)."]
    pub fn get_event_by_submitter(
        &self,
        submitter: Address,
        submitter_index: u32,
    ) -> Result<super::Event, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(submitter), Into::<Val>::into(submitter_index)];
        self.transport.invoke("get_event_by_submitter", args)
    }

    #[doc = "Read-only contract function `get_events_by_submitter`."]
    #[doc = "Wire name `submitter` (address)."]
    #[doc = "Wire name `start` (u32)."]
    #[doc = "Wire name `limit` (u32)."]
    pub fn get_events_by_submitter(
        &self,
        submitter: Address,
        start: u32,
        limit: u32,
    ) -> Result<Vec<super::Event>, TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(submitter),
            Into::<Val>::into(start),
            Into::<Val>::into(limit),
        ];
        self.transport.invoke("get_events_by_submitter", args)
    }

    #[doc = "Read-only contract function `get_events_by_time_range`."]
    #[doc = "Wire name `start_time` (u64)."]
    #[doc = "Wire name `end_time` (u64)."]
    #[doc = "Wire name `offset` (u32)."]
    #[doc = "Wire name `limit` (u32)."]
    pub fn get_events_by_time_range(
        &self,
        start_time: u64,
        end_time: u64,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<super::Event>, TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(start_time),
            Into::<Val>::into(end_time),
            Into::<Val>::into(offset),
            Into::<Val>::into(limit),
        ];
        self.transport.invoke("get_events_by_time_range", args)
    }

    #[doc = "Read-only contract function `search_events`."]
    #[doc = "Wire name `query` (bytes)."]
    #[doc = "Wire name `offset` (u32)."]
    #[doc = "Wire name `limit` (u32)."]
    pub fn search_events(&self, query: Bytes, offset: u32, limit: u32) -> Result<Vec<super::Event>, TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(query),
            Into::<Val>::into(offset),
            Into::<Val>::into(limit),
        ];
        self.transport.invoke("search_events", args)
    }

    #[doc = "State-changing contract function `update_event`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `index` (u32)."]
    #[doc = "Wire name `new_metadata` (bytes)."]
    pub fn update_event(&self, caller: Address, index: u32, new_metadata: Bytes) -> Result<BytesN<32>, TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(caller),
            Into::<Val>::into(index),
            Into::<Val>::into(new_metadata),
        ];
        self.transport.invoke("update_event", args)
    }

    #[doc = "Read-only contract function `get_event_history`."]
    #[doc = "Wire name `index` (u32)."]
    pub fn get_event_history(&self, index: u32) -> Result<Vec<super::EventVersion>, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(index)];
        self.transport.invoke("get_event_history", args)
    }

    #[doc = "State-changing contract function `rollback_event`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `index` (u32)."]
    #[doc = "Wire name `target_version` (u32)."]
    pub fn rollback_event(
        &self,
        caller: Address,
        index: u32,
        target_version: u32,
    ) -> Result<BytesN<32>, TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(caller),
            Into::<Val>::into(index),
            Into::<Val>::into(target_version),
        ];
        self.transport.invoke("rollback_event", args)
    }

    #[doc = "Read-only contract function `get_event_version_count`."]
    #[doc = "Wire name `index` (u32)."]
    pub fn get_event_version_count(&self, index: u32) -> Result<u32, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(index)];
        self.transport.invoke("get_event_version_count", args)
    }

    #[doc = "State-changing contract function `compare_event_versions`."]
    #[doc = "Wire name `index` (u32)."]
    #[doc = "Wire name `version_a` (u32)."]
    #[doc = "Wire name `version_b` (u32)."]
    pub fn compare_event_versions(&self, index: u32, version_a: u32, version_b: u32) -> Result<i32, TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(index),
            Into::<Val>::into(version_a),
            Into::<Val>::into(version_b),
        ];
        self.transport.invoke("compare_event_versions", args)
    }

    #[doc = "Read-only contract function `verify_integrity`."]
    pub fn verify_integrity(&self) -> Result<bool, TransportError> {
        let args: Vec<Val> = vec![];
        self.transport.invoke("verify_integrity", args)
    }

    #[doc = "Read-only contract function `verify_integrity_range`."]
    #[doc = "Wire name `from` (u32)."]
    #[doc = "Wire name `to` (u32)."]
    pub fn verify_integrity_range(&self, from: u32, to: u32) -> Result<bool, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(from), Into::<Val>::into(to)];
        self.transport.invoke("verify_integrity_range", args)
    }

    #[doc = "State-changing contract function `create_snapshot`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `description` (bytes)."]
    pub fn create_snapshot(&self, caller: Address, description: Bytes) -> Result<u32, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(caller), Into::<Val>::into(description)];
        self.transport.invoke("create_snapshot", args)
    }

    #[doc = "Read-only contract function `get_snapshot`."]
    #[doc = "Wire name `snapshot_id` (u32)."]
    pub fn get_snapshot(&self, snapshot_id: u32) -> Result<super::Snapshot, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(snapshot_id)];
        self.transport.invoke("get_snapshot", args)
    }

    #[doc = "Read-only contract function `snapshot_count`."]
    pub fn snapshot_count(&self) -> Result<u32, TransportError> {
        let args: Vec<Val> = vec![];
        self.transport.invoke("snapshot_count", args)
    }

    #[doc = "Read-only contract function `verify_snapshot`."]
    #[doc = "Wire name `snapshot_id` (u32)."]
    pub fn verify_snapshot(&self, snapshot_id: u32) -> Result<bool, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(snapshot_id)];
        self.transport.invoke("verify_snapshot", args)
    }

    #[doc = "State-changing contract function `cleanup_stale_hashes`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `start_index` (u32)."]
    #[doc = "Wire name `batch_size` (u32)."]
    pub fn cleanup_stale_hashes(
        &self,
        caller: Address,
        start_index: u32,
        batch_size: u32,
    ) -> Result<u32, TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(caller),
            Into::<Val>::into(start_index),
            Into::<Val>::into(batch_size),
        ];
        self.transport.invoke("cleanup_stale_hashes", args)
    }

    #[doc = "State-changing contract function `set_global_max_logs`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `new_max` (u32)."]
    pub fn set_global_max_logs(&self, caller: Address, new_max: u32) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(caller), Into::<Val>::into(new_max)];
        self.transport.invoke("set_global_max_logs", args)
    }

    #[doc = "State-changing contract function `set_event_max_logs`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `event_type` (symbol)."]
    #[doc = "Wire name `new_max` (u32)."]
    pub fn set_event_max_logs(&self, caller: Address, event_type: Symbol, new_max: u32) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(caller),
            Into::<Val>::into(event_type),
            Into::<Val>::into(new_max),
        ];
        self.transport.invoke("set_event_max_logs", args)
    }

    #[doc = "State-changing contract function `remove_event_cap`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `event_type` (symbol)."]
    pub fn remove_event_cap(&self, caller: Address, event_type: Symbol) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(caller), Into::<Val>::into(event_type)];
        self.transport.invoke("remove_event_cap", args)
    }

    #[doc = "Read-only contract function `has_cap`."]
    #[doc = "Wire name `event_type` (symbol)."]
    pub fn has_cap(&self, event_type: Symbol) -> Result<bool, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(event_type)];
        self.transport.invoke("has_cap", args)
    }

    #[doc = "State-changing contract function `transfer_ownership`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `new_owner` (address)."]
    pub fn transfer_ownership(&self, caller: Address, new_owner: Address) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(caller), Into::<Val>::into(new_owner)];
        self.transport.invoke("transfer_ownership", args)
    }

    #[doc = "State-changing contract function `set_metadata_max_size`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `max_size` (u32)."]
    pub fn set_metadata_max_size(&self, caller: Address, max_size: u32) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(caller), Into::<Val>::into(max_size)];
        self.transport.invoke("set_metadata_max_size", args)
    }

    #[doc = "State-changing contract function `set_event_metadata_max_size`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `event_type` (symbol)."]
    #[doc = "Wire name `max_size` (u32)."]
    pub fn set_event_metadata_max_size(
        &self,
        caller: Address,
        event_type: Symbol,
        max_size: u32,
    ) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(caller),
            Into::<Val>::into(event_type),
            Into::<Val>::into(max_size),
        ];
        self.transport.invoke("set_event_metadata_max_size", args)
    }

    #[doc = "State-changing contract function `set_metadata_schema`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `event_type` (symbol)."]
    #[doc = "Wire name `schema` (bytes)."]
    pub fn set_metadata_schema(
        &self,
        caller: Address,
        event_type: Symbol,
        schema: Bytes,
    ) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(caller),
            Into::<Val>::into(event_type),
            Into::<Val>::into(schema),
        ];
        self.transport.invoke("set_metadata_schema", args)
    }

    #[doc = "Read-only contract function `get_metadata_schema`."]
    #[doc = "Wire name `event_type` (symbol)."]
    pub fn get_metadata_schema(&self, event_type: Symbol) -> Result<Bytes, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(event_type)];
        self.transport.invoke("get_metadata_schema", args)
    }

    #[doc = "State-changing contract function `register_schema`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `event_type` (symbol)."]
    #[doc = "Wire name `schema` (Schema)."]
    #[doc = "Wire name `version` (u32)."]
    pub fn register_schema(
        &self,
        caller: Address,
        event_type: Symbol,
        schema: super::Schema,
        version: u32,
    ) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(caller),
            Into::<Val>::into(event_type),
            Into::<Val>::into(schema),
            Into::<Val>::into(version),
        ];
        self.transport.invoke("register_schema", args)
    }

    #[doc = "Read-only contract function `get_schema`."]
    #[doc = "Wire name `event_type` (symbol)."]
    #[doc = "Wire name `version` (u32)."]
    pub fn get_schema(&self, event_type: Symbol, version: u32) -> Result<Option<super::Schema>, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(event_type), Into::<Val>::into(version)];
        self.transport.invoke("get_schema", args)
    }

    #[doc = "Read-only contract function `list_schemas`."]
    #[doc = "Wire name `event_type` (symbol)."]
    pub fn list_schemas(&self, event_type: Symbol) -> Result<Vec<u32>, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(event_type)];
        self.transport.invoke("list_schemas", args)
    }

    #[doc = "State-changing contract function `migrate_event_metadata`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `event_type` (symbol)."]
    #[doc = "Wire name `from_version` (u32)."]
    #[doc = "Wire name `to_version` (u32)."]
    #[doc = "Wire name `migration_fn` (MigrationFunction)."]
    pub fn migrate_event_metadata(
        &self,
        caller: Address,
        event_type: Symbol,
        from_version: u32,
        to_version: u32,
        migration_fn: super::MigrationFunction,
    ) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(caller),
            Into::<Val>::into(event_type),
            Into::<Val>::into(from_version),
            Into::<Val>::into(to_version),
            Into::<Val>::into(migration_fn),
        ];
        self.transport.invoke("migrate_event_metadata", args)
    }

    #[doc = "Read-only contract function `get_migration_function`."]
    #[doc = "Wire name `event_type` (symbol)."]
    #[doc = "Wire name `from_version` (u32)."]
    #[doc = "Wire name `to_version` (u32)."]
    pub fn get_migration_function(
        &self,
        event_type: Symbol,
        from_version: u32,
        to_version: u32,
    ) -> Result<Option<super::MigrationFunction>, TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(event_type),
            Into::<Val>::into(from_version),
            Into::<Val>::into(to_version),
        ];
        self.transport.invoke("get_migration_function", args)
    }

    #[doc = "State-changing contract function `check_schema_compatibility`."]
    #[doc = "Wire name `left` (Schema)."]
    #[doc = "Wire name `right` (Schema)."]
    pub fn check_schema_compatibility(
        &self,
        left: super::Schema,
        right: super::Schema,
    ) -> Result<super::SchemaCompatibility, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(left), Into::<Val>::into(right)];
        self.transport.invoke("check_schema_compatibility", args)
    }

    #[doc = "Read-only contract function `is_backward_compatible`."]
    #[doc = "Wire name `left` (Schema)."]
    #[doc = "Wire name `right` (Schema)."]
    pub fn is_backward_compatible(&self, left: super::Schema, right: super::Schema) -> Result<bool, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(left), Into::<Val>::into(right)];
        self.transport.invoke("is_backward_compatible", args)
    }

    #[doc = "Read-only contract function `is_forward_compatible`."]
    #[doc = "Wire name `left` (Schema)."]
    #[doc = "Wire name `right` (Schema)."]
    pub fn is_forward_compatible(&self, left: super::Schema, right: super::Schema) -> Result<bool, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(left), Into::<Val>::into(right)];
        self.transport.invoke("is_forward_compatible", args)
    }

    #[doc = "State-changing contract function `set_event_ttl`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `ttl_ledgers` (u32)."]
    pub fn set_event_ttl(&self, caller: Address, ttl_ledgers: u32) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(caller), Into::<Val>::into(ttl_ledgers)];
        self.transport.invoke("set_event_ttl", args)
    }

    #[doc = "Read-only contract function `get_event_ttl`."]
    pub fn get_event_ttl(&self) -> Result<u32, TransportError> {
        let args: Vec<Val> = vec![];
        self.transport.invoke("get_event_ttl", args)
    }

    #[doc = "State-changing contract function `cleanup_expired_events`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `start_index` (u32)."]
    #[doc = "Wire name `batch_size` (u32)."]
    pub fn cleanup_expired_events(
        &self,
        caller: Address,
        start_index: u32,
        batch_size: u32,
    ) -> Result<u32, TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(caller),
            Into::<Val>::into(start_index),
            Into::<Val>::into(batch_size),
        ];
        self.transport.invoke("cleanup_expired_events", args)
    }

    #[doc = "Read-only contract function `get_cleanup_stats`."]
    pub fn get_cleanup_stats(&self) -> Result<super::TtlCleanupStats, TransportError> {
        let args: Vec<Val> = vec![];
        self.transport.invoke("get_cleanup_stats", args)
    }

    #[doc = "State-changing contract function `register_webhook`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `event_type` (symbol)."]
    #[doc = "Wire name `url` (bytes)."]
    #[doc = "Wire name `secret` (bytes)."]
    pub fn register_webhook(
        &self,
        caller: Address,
        event_type: Symbol,
        url: Bytes,
        secret: Bytes,
    ) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(caller),
            Into::<Val>::into(event_type),
            Into::<Val>::into(url),
            Into::<Val>::into(secret),
        ];
        self.transport.invoke("register_webhook", args)
    }

    #[doc = "State-changing contract function `unregister_webhook`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `event_type` (symbol)."]
    #[doc = "Wire name `url` (bytes)."]
    pub fn unregister_webhook(&self, caller: Address, event_type: Symbol, url: Bytes) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(caller),
            Into::<Val>::into(event_type),
            Into::<Val>::into(url),
        ];
        self.transport.invoke("unregister_webhook", args)
    }

    #[doc = "Read-only contract function `get_webhooks`."]
    #[doc = "Wire name `event_type` (symbol)."]
    pub fn get_webhooks(&self, event_type: Symbol) -> Result<Vec<Bytes>, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(event_type)];
        self.transport.invoke("get_webhooks", args)
    }

    #[doc = "State-changing contract function `pause`."]
    #[doc = "Wire name `caller` (address)."]
    pub fn pause(&self, caller: Address) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(caller)];
        self.transport.invoke("pause", args)
    }

    #[doc = "State-changing contract function `unpause`."]
    #[doc = "Wire name `caller` (address)."]
    pub fn unpause(&self, caller: Address) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(caller)];
        self.transport.invoke("unpause", args)
    }

    #[doc = "Read-only contract function `is_paused`."]
    pub fn is_paused(&self) -> Result<bool, TransportError> {
        let args: Vec<Val> = vec![];
        self.transport.invoke("is_paused", args)
    }

    #[doc = "State-changing contract function `paused_since`."]
    pub fn paused_since(&self) -> Result<u64, TransportError> {
        let args: Vec<Val> = vec![];
        self.transport.invoke("paused_since", args)
    }

    #[doc = "State-changing contract function `set_category_max_len`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `max_len` (u32)."]
    pub fn set_category_max_len(&self, caller: Address, max_len: u32) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(caller), Into::<Val>::into(max_len)];
        self.transport.invoke("set_category_max_len", args)
    }

    #[doc = "State-changing contract function `block_submitter`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `submitter` (address)."]
    pub fn block_submitter(&self, caller: Address, submitter: Address) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(caller), Into::<Val>::into(submitter)];
        self.transport.invoke("block_submitter", args)
    }

    #[doc = "State-changing contract function `unblock_submitter`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `submitter` (address)."]
    pub fn unblock_submitter(&self, caller: Address, submitter: Address) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(caller), Into::<Val>::into(submitter)];
        self.transport.invoke("unblock_submitter", args)
    }

    #[doc = "State-changing contract function `enable_allowlist_mode`."]
    #[doc = "Wire name `caller` (address)."]
    pub fn enable_allowlist_mode(&self, caller: Address) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(caller)];
        self.transport.invoke("enable_allowlist_mode", args)
    }

    #[doc = "State-changing contract function `disable_allowlist_mode`."]
    #[doc = "Wire name `caller` (address)."]
    pub fn disable_allowlist_mode(&self, caller: Address) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(caller)];
        self.transport.invoke("disable_allowlist_mode", args)
    }

    #[doc = "State-changing contract function `allow_submitter`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `submitter` (address)."]
    pub fn allow_submitter(&self, caller: Address, submitter: Address) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(caller), Into::<Val>::into(submitter)];
        self.transport.invoke("allow_submitter", args)
    }

    #[doc = "State-changing contract function `remove_submitter_from_allowlist`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `submitter` (address)."]
    pub fn remove_submitter_from_allowlist(&self, caller: Address, submitter: Address) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(caller), Into::<Val>::into(submitter)];
        self.transport.invoke("remove_submitter_from_allowlist", args)
    }

    #[doc = "Read-only contract function `get_metadata_max_size`."]
    #[doc = "Wire name `event_type` (symbol)."]
    pub fn get_metadata_max_size(&self, event_type: Symbol) -> Result<u32, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(event_type)];
        self.transport.invoke("get_metadata_max_size", args)
    }

    #[doc = "Read-only contract function `get_statistics`."]
    #[doc = "Wire name `caller` (address)."]
    pub fn get_statistics(&self, caller: Address) -> Result<super::ContractStatistics, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(caller)];
        self.transport.invoke("get_statistics", args)
    }

    #[doc = "State-changing contract function `set_event_emission_mode`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `mode` (u32)."]
    pub fn set_event_emission_mode(&self, caller: Address, mode: u32) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(caller), Into::<Val>::into(mode)];
        self.transport.invoke("set_event_emission_mode", args)
    }

    #[doc = "Read-only contract function `get_event_emission_mode`."]
    pub fn get_event_emission_mode(&self) -> Result<u32, TransportError> {
        let args: Vec<Val> = vec![];
        self.transport.invoke("get_event_emission_mode", args)
    }

    #[doc = "State-changing contract function `set_low_cost_mode`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `enabled` (bool)."]
    pub fn set_low_cost_mode(&self, caller: Address, enabled: bool) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(caller), Into::<Val>::into(enabled)];
        self.transport.invoke("set_low_cost_mode", args)
    }

    #[doc = "Read-only contract function `is_low_cost_mode`."]
    pub fn is_low_cost_mode(&self) -> Result<bool, TransportError> {
        let args: Vec<Val> = vec![];
        self.transport.invoke("is_low_cost_mode", args)
    }

    #[doc = "State-changing contract function `set_submitter_rate_limit`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `submitter` (address)."]
    #[doc = "Wire name `max_per_timestamp` (u32)."]
    pub fn set_submitter_rate_limit(
        &self,
        caller: Address,
        submitter: Address,
        max_per_timestamp: u32,
    ) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(caller),
            Into::<Val>::into(submitter),
            Into::<Val>::into(max_per_timestamp),
        ];
        self.transport.invoke("set_submitter_rate_limit", args)
    }

    #[doc = "State-changing contract function `compact_storage`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `stale_types` (vec<symbol>)."]
    pub fn compact_storage(&self, caller: Address, stale_types: Vec<Symbol>) -> Result<u32, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(caller), Into::<Val>::into(stale_types)];
        self.transport.invoke("compact_storage", args)
    }

    #[doc = "State-changing contract function `log_event_signed`."]
    #[doc = "Wire name `submitter` (address)."]
    #[doc = "Wire name `event_type` (symbol)."]
    #[doc = "Wire name `metadata` (bytes)."]
    #[doc = "Wire name `signature_payload` (bytes)."]
    pub fn log_event_signed(
        &self,
        submitter: Address,
        event_type: Symbol,
        metadata: Bytes,
        signature_payload: Bytes,
    ) -> Result<BytesN<32>, TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(submitter),
            Into::<Val>::into(event_type),
            Into::<Val>::into(metadata),
            Into::<Val>::into(signature_payload),
        ];
        self.transport.invoke("log_event_signed", args)
    }

    #[doc = "Read-only contract function `get_event_signature`."]
    #[doc = "Wire name `event_id` (bytesn<32>)."]
    pub fn get_event_signature(&self, event_id: BytesN<32>) -> Result<Option<Bytes>, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(event_id)];
        self.transport.invoke("get_event_signature", args)
    }

    #[doc = "Read-only contract function `find_event_by_content`."]
    #[doc = "Wire name `event_type` (symbol)."]
    #[doc = "Wire name `submitter` (address)."]
    #[doc = "Wire name `metadata` (bytes)."]
    pub fn find_event_by_content(
        &self,
        event_type: Symbol,
        submitter: Address,
        metadata: Bytes,
    ) -> Result<Option<super::Event>, TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(event_type),
            Into::<Val>::into(submitter),
            Into::<Val>::into(metadata),
        ];
        self.transport.invoke("find_event_by_content", args)
    }

    #[doc = "State-changing contract function `add_owner`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `new_owner` (address)."]
    pub fn add_owner(&self, caller: Address, new_owner: Address) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(caller), Into::<Val>::into(new_owner)];
        self.transport.invoke("add_owner", args)
    }

    #[doc = "State-changing contract function `remove_owner`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `owner_to_remove` (address)."]
    pub fn remove_owner(&self, caller: Address, owner_to_remove: Address) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(caller), Into::<Val>::into(owner_to_remove)];
        self.transport.invoke("remove_owner", args)
    }

    #[doc = "State-changing contract function `set_required_signatures`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `required` (u32)."]
    pub fn set_required_signatures(&self, caller: Address, required: u32) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(caller), Into::<Val>::into(required)];
        self.transport.invoke("set_required_signatures", args)
    }

    #[doc = "State-changing contract function `submit_proposal`."]
    #[doc = "Wire name `proposer` (address)."]
    #[doc = "Wire name `action` (ProposalAction)."]
    #[doc = "Wire name `ttl_seconds` (u64)."]
    pub fn submit_proposal(
        &self,
        proposer: Address,
        action: super::ProposalAction,
        ttl_seconds: u64,
    ) -> Result<u32, TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(proposer),
            Into::<Val>::into(action),
            Into::<Val>::into(ttl_seconds),
        ];
        self.transport.invoke("submit_proposal", args)
    }

    #[doc = "State-changing contract function `approve_proposal`."]
    #[doc = "Wire name `approver` (address)."]
    #[doc = "Wire name `proposal_id` (u32)."]
    pub fn approve_proposal(&self, approver: Address, proposal_id: u32) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(approver), Into::<Val>::into(proposal_id)];
        self.transport.invoke("approve_proposal", args)
    }

    #[doc = "State-changing contract function `execute_proposal`."]
    #[doc = "Wire name `executor` (address)."]
    #[doc = "Wire name `proposal_id` (u32)."]
    pub fn execute_proposal(&self, executor: Address, proposal_id: u32) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(executor), Into::<Val>::into(proposal_id)];
        self.transport.invoke("execute_proposal", args)
    }

    #[doc = "State-changing contract function `set_role`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `target` (address)."]
    #[doc = "Wire name `role` (option<Role>)."]
    pub fn set_role(&self, caller: Address, target: Address, role: Option<super::Role>) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(caller),
            Into::<Val>::into(target),
            Into::<Val>::into(role),
        ];
        self.transport.invoke("set_role", args)
    }

    #[doc = "Read-only contract function `get_role`."]
    #[doc = "Wire name `target` (address)."]
    pub fn get_role(&self, target: Address) -> Result<Option<super::Role>, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(target)];
        self.transport.invoke("get_role", args)
    }

    #[doc = "State-changing contract function `enable_rbac`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `enabled` (bool)."]
    pub fn enable_rbac(&self, caller: Address, enabled: bool) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(caller), Into::<Val>::into(enabled)];
        self.transport.invoke("enable_rbac", args)
    }

    #[doc = "Read-only contract function `is_rbac_enabled`."]
    pub fn is_rbac_enabled(&self) -> Result<bool, TransportError> {
        let args: Vec<Val> = vec![];
        self.transport.invoke("is_rbac_enabled", args)
    }

    #[doc = "State-changing contract function `set_dedup_policy`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `policy` (DedupPolicy)."]
    pub fn set_dedup_policy(&self, caller: Address, policy: super::DedupPolicy) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(caller), Into::<Val>::into(policy)];
        self.transport.invoke("set_dedup_policy", args)
    }

    #[doc = "Read-only contract function `get_dedup_policy`."]
    pub fn get_dedup_policy(&self) -> Result<super::DedupPolicy, TransportError> {
        let args: Vec<Val> = vec![];
        self.transport.invoke("get_dedup_policy", args)
    }

    #[doc = "State-changing contract function `set_dedup_policy_for_type`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `event_type` (symbol)."]
    #[doc = "Wire name `policy` (option<DedupPolicy>)."]
    pub fn set_dedup_policy_for_type(
        &self,
        caller: Address,
        event_type: Symbol,
        policy: Option<super::DedupPolicy>,
    ) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(caller),
            Into::<Val>::into(event_type),
            Into::<Val>::into(policy),
        ];
        self.transport.invoke("set_dedup_policy_for_type", args)
    }

    #[doc = "Read-only contract function `get_dedup_policy_for_type`."]
    #[doc = "Wire name `event_type` (symbol)."]
    pub fn get_dedup_policy_for_type(&self, event_type: Symbol) -> Result<super::DedupPolicy, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(event_type)];
        self.transport.invoke("get_dedup_policy_for_type", args)
    }

    #[doc = "State-changing contract function `log_event_with_custom_key`."]
    #[doc = "Wire name `submitter` (address)."]
    #[doc = "Wire name `event_type` (symbol)."]
    #[doc = "Wire name `metadata` (bytes)."]
    #[doc = "Wire name `category` (option<symbol>)."]
    #[doc = "Wire name `sub_event_type` (option<symbol>)."]
    #[doc = "Wire name `force` (bool)."]
    #[doc = "Wire name `custom_key` (option<bytesn<32>>)."]
    pub fn log_event_with_custom_key(
        &self,
        submitter: Address,
        event_type: Symbol,
        metadata: Bytes,
        category: Option<Symbol>,
        sub_event_type: Option<Symbol>,
        force: bool,
        custom_key: Option<BytesN<32>>,
    ) -> Result<BytesN<32>, TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(submitter),
            Into::<Val>::into(event_type),
            Into::<Val>::into(metadata),
            Into::<Val>::into(category),
            Into::<Val>::into(sub_event_type),
            Into::<Val>::into(force),
            Into::<Val>::into(custom_key),
        ];
        self.transport.invoke("log_event_with_custom_key", args)
    }

    #[doc = "State-changing contract function `cleanup_stale_dedup_entries`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `start_index` (u32)."]
    #[doc = "Wire name `batch_size` (u32)."]
    pub fn cleanup_stale_dedup_entries(
        &self,
        caller: Address,
        start_index: u32,
        batch_size: u32,
    ) -> Result<u32, TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(caller),
            Into::<Val>::into(start_index),
            Into::<Val>::into(batch_size),
        ];
        self.transport.invoke("cleanup_stale_dedup_entries", args)
    }

    #[doc = "State-changing contract function `set_archive_config`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `config` (ArchiveConfig)."]
    pub fn set_archive_config(&self, caller: Address, config: super::ArchiveConfig) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(caller), Into::<Val>::into(config)];
        self.transport.invoke("set_archive_config", args)
    }

    #[doc = "Read-only contract function `get_archive_config`."]
    pub fn get_archive_config(&self) -> Result<Option<super::ArchiveConfig>, TransportError> {
        let args: Vec<Val> = vec![];
        self.transport.invoke("get_archive_config", args)
    }

    #[doc = "Read-only contract function `get_event_audit_trail`."]
    #[doc = "Wire name `index` (u32)."]
    pub fn get_event_audit_trail(&self, index: u32) -> Result<Vec<super::EventVersion>, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(index)];
        self.transport.invoke("get_event_audit_trail", args)
    }

    #[doc = "State-changing contract function `tag_event_version`."]
    #[doc = "Wire name `caller` (address)."]
    #[doc = "Wire name `index` (u32)."]
    #[doc = "Wire name `version` (u32)."]
    #[doc = "Wire name `tag` (symbol)."]
    pub fn tag_event_version(
        &self,
        caller: Address,
        index: u32,
        version: u32,
        tag: Symbol,
    ) -> Result<(), TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(caller),
            Into::<Val>::into(index),
            Into::<Val>::into(version),
            Into::<Val>::into(tag),
        ];
        self.transport.invoke("tag_event_version", args)
    }

    #[doc = "Read-only contract function `get_event_version_tag`."]
    #[doc = "Wire name `index` (u32)."]
    #[doc = "Wire name `version` (u32)."]
    pub fn get_event_version_tag(&self, index: u32, version: u32) -> Result<Option<Symbol>, TransportError> {
        let args: Vec<Val> = vec![Into::<Val>::into(index), Into::<Val>::into(version)];
        self.transport.invoke("get_event_version_tag", args)
    }

    #[doc = "Read-only contract function `get_event_diff`."]
    #[doc = "Wire name `index` (u32)."]
    #[doc = "Wire name `from_version` (u32)."]
    #[doc = "Wire name `to_version` (u32)."]
    pub fn get_event_diff(
        &self,
        index: u32,
        from_version: u32,
        to_version: u32,
    ) -> Result<Vec<super::FieldChange>, TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(index),
            Into::<Val>::into(from_version),
            Into::<Val>::into(to_version),
        ];
        self.transport.invoke("get_event_diff", args)
    }

    #[doc = "State-changing contract function `compare_event_versions_detailed`."]
    #[doc = "Wire name `index` (u32)."]
    #[doc = "Wire name `from_version` (u32)."]
    #[doc = "Wire name `to_version` (u32)."]
    pub fn compare_event_versions_detailed(
        &self,
        index: u32,
        from_version: u32,
        to_version: u32,
    ) -> Result<super::VersionComparison, TransportError> {
        let args: Vec<Val> = vec![
            Into::<Val>::into(index),
            Into::<Val>::into(from_version),
            Into::<Val>::into(to_version),
        ];
        self.transport.invoke("compare_event_versions_detailed", args)
    }
}

// contract: AuditLedger v0.1.0
// idl: spec_version 1.0.0 (120 functions, 19 types, 75 errors, 43 events)
