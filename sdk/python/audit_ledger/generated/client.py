# GENERATED FILE — DO NOT EDIT.
# Regenerate with: npm run generate --workspace tools/sdk-codegen (or `sdk-codegen generate`).
# Source of truth: abi/audit-ledger.json
"""

Type-safe client for the AuditLedger Soroban contract.

The client is transport-agnostic: supply any object implementing
`AuditLedgerTransport` (Stellar RPC, a local `Env` in tests, or a mock).
Argument and return types are derived from the contract IDL, so a contract
change that is not regenerated here fails at import time rather than at
runtime.
"""

from typing import Any, Dict, List, Optional, Protocol, Sequence, Tuple, Union, runtime_checkable

from .types import ArchiveConfig, ArchiveStats, ArchivedEventRef, ContractStatistics, DedupPolicy, Event, EventHeader, EventVersion, FieldChange, MigrationFunction, NonceState, ProposalAction, Role, Schema, SchemaCompatibility, Snapshot, TtlCleanupStats, VersionComparison

__all__ = ["AuditLedgerTransport", "GeneratedAuditLedgerClient"]


@runtime_checkable
class AuditLedgerTransport(Protocol):
    """Invokes a contract function and returns its decoded result."""

    def invoke(self, method: str, args: Sequence[Any]) -> Any:
        """Invoke `method` with positional `args` and return the result."""
        ...


class GeneratedAuditLedgerClient:
    """Generated client binding every public function of AuditLedger."""

    #: Contract function names exposed by this client.
    FUNCTION_NAMES: Sequence[str] = (
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
    )

    def __init__(self, contract_id: str, transport: AuditLedgerTransport) -> None:
        self.contract_id = contract_id
        self._transport = transport

    def initialize(self, owners: List[str], global_max_logs: int, max_metadata_bytes: int) -> None:
        """State-changing contract function `initialize`.

        Args:
            owners: wire name `owners` (vec<address>).
            global_max_logs: wire name `global_max_logs` (u32).
            max_metadata_bytes: wire name `max_metadata_bytes` (u32).
        """

        return self._transport.invoke("initialize", [owners, global_max_logs, max_metadata_bytes])

    def log_events(self, events: List[Tuple[str, str, str]]) -> List[int]:
        """State-changing contract function `log_events`.

        Args:
            events: wire name `events` (vec<tuple<address,symbol,bytes>>).
        Returns:
            vec<u32>.
        """

        return self._transport.invoke("log_events", [events])

    def log_event(self, submitter: str, event_type: str, metadata: str, category: Optional[str], sub_event_type: Optional[str], force: bool) -> str:
        """State-changing contract function `log_event`.

        Args:
            submitter: wire name `submitter` (address).
            event_type: wire name `event_type` (symbol).
            metadata: wire name `metadata` (bytes).
            category: wire name `category` (option<symbol>).
            sub_event_type: wire name `sub_event_type` (option<symbol>).
            force: wire name `force` (bool).
        Returns:
            bytesn<32>.
        """

        return self._transport.invoke("log_event", [submitter, event_type, metadata, category, sub_event_type, force])

    def log_event_with_hierarchy(self, submitter: str, event_type: str, metadata: str, category: Optional[str], sub_event_type: Optional[str], force: bool) -> str:
        """State-changing contract function `log_event_with_hierarchy`.

        Args:
            submitter: wire name `submitter` (address).
            event_type: wire name `event_type` (symbol).
            metadata: wire name `metadata` (bytes).
            category: wire name `category` (option<symbol>).
            sub_event_type: wire name `sub_event_type` (option<symbol>).
            force: wire name `force` (bool).
        Returns:
            bytesn<32>.
        """

        return self._transport.invoke("log_event_with_hierarchy", [submitter, event_type, metadata, category, sub_event_type, force])

    def log_event_with_nonce(self, submitter: str, event_type: str, metadata: str, nonce: int) -> str:
        """State-changing contract function `log_event_with_nonce`.

        Args:
            submitter: wire name `submitter` (address).
            event_type: wire name `event_type` (symbol).
            metadata: wire name `metadata` (bytes).
            nonce: wire name `nonce` (u32).
        Returns:
            bytesn<32>.
        """

        return self._transport.invoke("log_event_with_nonce", [submitter, event_type, metadata, nonce])

    def get_submitter_nonce(self, submitter: str) -> int:
        """Read-only contract function `get_submitter_nonce`.

        Args:
            submitter: wire name `submitter` (address).
        Returns:
            u32.
        """

        return self._transport.invoke("get_submitter_nonce", [submitter])

    def get_submitter_nonce_state(self, submitter: str) -> NonceState:
        """Read-only contract function `get_submitter_nonce_state`.

        Args:
            submitter: wire name `submitter` (address).
        Returns:
            NonceState.
        """

        return self._transport.invoke("get_submitter_nonce_state", [submitter])

    def set_submitter_nonce_config(self, caller: str, submitter: str, window_size: int, max_nonce: int) -> None:
        """State-changing contract function `set_submitter_nonce_config`.

        Args:
            caller: wire name `caller` (address).
            submitter: wire name `submitter` (address).
            window_size: wire name `window_size` (u32).
            max_nonce: wire name `max_nonce` (u32).
        """

        return self._transport.invoke("set_submitter_nonce_config", [caller, submitter, window_size, max_nonce])

    def reset_submitter_nonce(self, caller: str, submitter: str) -> None:
        """State-changing contract function `reset_submitter_nonce`.

        Args:
            caller: wire name `caller` (address).
            submitter: wire name `submitter` (address).
        """

        return self._transport.invoke("reset_submitter_nonce", [caller, submitter])

    def set_default_nonce_config(self, caller: str, window_size: int, max_nonce: int) -> None:
        """State-changing contract function `set_default_nonce_config`.

        Args:
            caller: wire name `caller` (address).
            window_size: wire name `window_size` (u32).
            max_nonce: wire name `max_nonce` (u32).
        """

        return self._transport.invoke("set_default_nonce_config", [caller, window_size, max_nonce])

    def get_default_nonce_window_size(self) -> int:
        """Read-only contract function `get_default_nonce_window_size`.

        Returns:
            u32.
        """

        return self._transport.invoke("get_default_nonce_window_size", [])

    def get_default_nonce_max_value(self) -> int:
        """Read-only contract function `get_default_nonce_max_value`.

        Returns:
            u32.
        """

        return self._transport.invoke("get_default_nonce_max_value", [])

    def total_events(self) -> int:
        """Read-only contract function `total_events`.

        Returns:
            u32.
        """

        return self._transport.invoke("total_events", [])

    def get_event_type_count(self, event_type: str) -> int:
        """Read-only contract function `get_event_type_count`.

        Args:
            event_type: wire name `event_type` (symbol).
        Returns:
            u32.
        """

        return self._transport.invoke("get_event_type_count", [event_type])

    def get_event(self, id_: str) -> Event:
        """Read-only contract function `get_event`.

        Args:
            id: wire name `id` (bytesn<32>).
        Returns:
            Event.
        """

        return self._transport.invoke("get_event", [id_])

    def get_event_metadata(self, id_: str) -> str:
        """Read-only contract function `get_event_metadata`.

        Args:
            id: wire name `id` (bytesn<32>).
        Returns:
            bytes.
        """

        return self._transport.invoke("get_event_metadata", [id_])

    def get_event_header(self, id_: str) -> EventHeader:
        """Read-only contract function `get_event_header`.

        Args:
            id: wire name `id` (bytesn<32>).
        Returns:
            EventHeader.
        """

        return self._transport.invoke("get_event_header", [id_])

    def get_event_by_order(self, order: int) -> Event:
        """Read-only contract function `get_event_by_order`.

        Args:
            order: wire name `order` (u32).
        Returns:
            Event.
        """

        return self._transport.invoke("get_event_by_order", [order])

    def event_count(self, event_type: str) -> int:
        """Read-only contract function `event_count`.

        Args:
            event_type: wire name `event_type` (symbol).
        Returns:
            u32.
        """

        return self._transport.invoke("event_count", [event_type])

    def event_count_by_category(self, category: str) -> int:
        """Read-only contract function `event_count_by_category`.

        Args:
            category: wire name `category` (symbol).
        Returns:
            u32.
        """

        return self._transport.invoke("event_count_by_category", [category])

    def list_events_by_category(self, category: str, start: int, limit: int) -> List[EventHeader]:
        """Read-only contract function `list_events_by_category`.

        Args:
            category: wire name `category` (symbol).
            start: wire name `start` (u32).
            limit: wire name `limit` (u32).
        Returns:
            vec<EventHeader>.
        """

        return self._transport.invoke("list_events_by_category", [category, start, limit])

    def archive_events(self, caller: str, cutoff_timestamp: int) -> int:
        """State-changing contract function `archive_events`.

        Args:
            caller: wire name `caller` (address).
            cutoff_timestamp: wire name `cutoff_timestamp` (u64).
        Returns:
            u32.
        """

        return self._transport.invoke("archive_events", [caller, cutoff_timestamp])

    def get_archived_event(self, id_: str) -> Event:
        """Read-only contract function `get_archived_event`.

        Args:
            id: wire name `id` (bytesn<32>).
        Returns:
            Event.
        """

        return self._transport.invoke("get_archived_event", [id_])

    def get_archived_event_ref(self, id_: str) -> Optional[ArchivedEventRef]:
        """Read-only contract function `get_archived_event_ref`.

        Args:
            id: wire name `id` (bytesn<32>).
        Returns:
            option<ArchivedEventRef>.
        """

        return self._transport.invoke("get_archived_event_ref", [id_])

    def verify_archived_event_checksum(self, id_: str, candidate: Event) -> bool:
        """Read-only contract function `verify_archived_event_checksum`.

        Args:
            id: wire name `id` (bytesn<32>).
            candidate: wire name `candidate` (Event).
        Returns:
            bool.
        """

        return self._transport.invoke("verify_archived_event_checksum", [id_, candidate])

    def get_archive_stats(self) -> ArchiveStats:
        """Read-only contract function `get_archive_stats`.

        Returns:
            ArchiveStats.
        """

        return self._transport.invoke("get_archive_stats", [])

    def get_archived_event_count(self) -> int:
        """Read-only contract function `get_archived_event_count`.

        Returns:
            u32.
        """

        return self._transport.invoke("get_archived_event_count", [])

    def list_archived_events(self, start: int, limit: int) -> List[EventHeader]:
        """Read-only contract function `list_archived_events`.

        Args:
            start: wire name `start` (u32).
            limit: wire name `limit` (u32).
        Returns:
            vec<EventHeader>.
        """

        return self._transport.invoke("list_archived_events", [start, limit])

    def purge_archived_events(self, caller: str, cutoff_timestamp: int, confirm: bool) -> int:
        """State-changing contract function `purge_archived_events`.

        Args:
            caller: wire name `caller` (address).
            cutoff_timestamp: wire name `cutoff_timestamp` (u64).
            confirm: wire name `confirm` (bool).
        Returns:
            u32.
        """

        return self._transport.invoke("purge_archived_events", [caller, cutoff_timestamp, confirm])

    def upgrade_contract(self, caller: str, new_wasm_hash: str) -> None:
        """State-changing contract function `upgrade_contract`.

        Args:
            caller: wire name `caller` (address).
            new_wasm_hash: wire name `new_wasm_hash` (bytesn<32>).
        """

        return self._transport.invoke("upgrade_contract", [caller, new_wasm_hash])

    def get_event_by_type(self, event_type: str, type_index: int) -> Event:
        """Read-only contract function `get_event_by_type`.

        Args:
            event_type: wire name `event_type` (symbol).
            type_index: wire name `type_index` (u32).
        Returns:
            Event.
        """

        return self._transport.invoke("get_event_by_type", [event_type, type_index])

    def list_events(self, offset: int, limit: int) -> List[Event]:
        """Read-only contract function `list_events`.

        Args:
            offset: wire name `offset` (u32).
            limit: wire name `limit` (u32).
        Returns:
            vec<Event>.
        """

        return self._transport.invoke("list_events", [offset, limit])

    def list_events_by_type(self, event_type: str, offset: int, limit: int) -> List[Event]:
        """Read-only contract function `list_events_by_type`.

        Args:
            event_type: wire name `event_type` (symbol).
            offset: wire name `offset` (u32).
            limit: wire name `limit` (u32).
        Returns:
            vec<Event>.
        """

        return self._transport.invoke("list_events_by_type", [event_type, offset, limit])

    def get_events_by_type(self, event_type: str, start: int, limit: int) -> List[Event]:
        """Read-only contract function `get_events_by_type`.

        Args:
            event_type: wire name `event_type` (symbol).
            start: wire name `start` (u32).
            limit: wire name `limit` (u32).
        Returns:
            vec<Event>.
        """

        return self._transport.invoke("get_events_by_type", [event_type, start, limit])

    def submitter_event_count(self, submitter: str) -> int:
        """Read-only contract function `submitter_event_count`.

        Args:
            submitter: wire name `submitter` (address).
        Returns:
            u32.
        """

        return self._transport.invoke("submitter_event_count", [submitter])

    def get_event_by_submitter(self, submitter: str, submitter_index: int) -> Event:
        """Read-only contract function `get_event_by_submitter`.

        Args:
            submitter: wire name `submitter` (address).
            submitter_index: wire name `submitter_index` (u32).
        Returns:
            Event.
        """

        return self._transport.invoke("get_event_by_submitter", [submitter, submitter_index])

    def get_events_by_submitter(self, submitter: str, start: int, limit: int) -> List[Event]:
        """Read-only contract function `get_events_by_submitter`.

        Args:
            submitter: wire name `submitter` (address).
            start: wire name `start` (u32).
            limit: wire name `limit` (u32).
        Returns:
            vec<Event>.
        """

        return self._transport.invoke("get_events_by_submitter", [submitter, start, limit])

    def get_events_by_time_range(self, start_time: int, end_time: int, offset: int, limit: int) -> List[Event]:
        """Read-only contract function `get_events_by_time_range`.

        Args:
            start_time: wire name `start_time` (u64).
            end_time: wire name `end_time` (u64).
            offset: wire name `offset` (u32).
            limit: wire name `limit` (u32).
        Returns:
            vec<Event>.
        """

        return self._transport.invoke("get_events_by_time_range", [start_time, end_time, offset, limit])

    def search_events(self, query: str, offset: int, limit: int) -> List[Event]:
        """Read-only contract function `search_events`.

        Args:
            query: wire name `query` (bytes).
            offset: wire name `offset` (u32).
            limit: wire name `limit` (u32).
        Returns:
            vec<Event>.
        """

        return self._transport.invoke("search_events", [query, offset, limit])

    def update_event(self, caller: str, index: int, new_metadata: str) -> str:
        """State-changing contract function `update_event`.

        Args:
            caller: wire name `caller` (address).
            index: wire name `index` (u32).
            new_metadata: wire name `new_metadata` (bytes).
        Returns:
            bytesn<32>.
        """

        return self._transport.invoke("update_event", [caller, index, new_metadata])

    def get_event_history(self, index: int) -> List[EventVersion]:
        """Read-only contract function `get_event_history`.

        Args:
            index: wire name `index` (u32).
        Returns:
            vec<EventVersion>.
        """

        return self._transport.invoke("get_event_history", [index])

    def rollback_event(self, caller: str, index: int, target_version: int) -> str:
        """State-changing contract function `rollback_event`.

        Args:
            caller: wire name `caller` (address).
            index: wire name `index` (u32).
            target_version: wire name `target_version` (u32).
        Returns:
            bytesn<32>.
        """

        return self._transport.invoke("rollback_event", [caller, index, target_version])

    def get_event_version_count(self, index: int) -> int:
        """Read-only contract function `get_event_version_count`.

        Args:
            index: wire name `index` (u32).
        Returns:
            u32.
        """

        return self._transport.invoke("get_event_version_count", [index])

    def compare_event_versions(self, index: int, version_a: int, version_b: int) -> int:
        """State-changing contract function `compare_event_versions`.

        Args:
            index: wire name `index` (u32).
            version_a: wire name `version_a` (u32).
            version_b: wire name `version_b` (u32).
        Returns:
            i32.
        """

        return self._transport.invoke("compare_event_versions", [index, version_a, version_b])

    def verify_integrity(self) -> bool:
        """Read-only contract function `verify_integrity`.

        Returns:
            bool.
        """

        return self._transport.invoke("verify_integrity", [])

    def verify_integrity_range(self, from_: int, to: int) -> bool:
        """Read-only contract function `verify_integrity_range`.

        Args:
            from: wire name `from` (u32).
            to: wire name `to` (u32).
        Returns:
            bool.
        """

        return self._transport.invoke("verify_integrity_range", [from_, to])

    def create_snapshot(self, caller: str, description: str) -> int:
        """State-changing contract function `create_snapshot`.

        Args:
            caller: wire name `caller` (address).
            description: wire name `description` (bytes).
        Returns:
            u32.
        """

        return self._transport.invoke("create_snapshot", [caller, description])

    def get_snapshot(self, snapshot_id: int) -> Snapshot:
        """Read-only contract function `get_snapshot`.

        Args:
            snapshot_id: wire name `snapshot_id` (u32).
        Returns:
            Snapshot.
        """

        return self._transport.invoke("get_snapshot", [snapshot_id])

    def snapshot_count(self) -> int:
        """Read-only contract function `snapshot_count`.

        Returns:
            u32.
        """

        return self._transport.invoke("snapshot_count", [])

    def verify_snapshot(self, snapshot_id: int) -> bool:
        """Read-only contract function `verify_snapshot`.

        Args:
            snapshot_id: wire name `snapshot_id` (u32).
        Returns:
            bool.
        """

        return self._transport.invoke("verify_snapshot", [snapshot_id])

    def cleanup_stale_hashes(self, caller: str, start_index: int, batch_size: int) -> int:
        """State-changing contract function `cleanup_stale_hashes`.

        Args:
            caller: wire name `caller` (address).
            start_index: wire name `start_index` (u32).
            batch_size: wire name `batch_size` (u32).
        Returns:
            u32.
        """

        return self._transport.invoke("cleanup_stale_hashes", [caller, start_index, batch_size])

    def set_global_max_logs(self, caller: str, new_max: int) -> None:
        """State-changing contract function `set_global_max_logs`.

        Args:
            caller: wire name `caller` (address).
            new_max: wire name `new_max` (u32).
        """

        return self._transport.invoke("set_global_max_logs", [caller, new_max])

    def set_event_max_logs(self, caller: str, event_type: str, new_max: int) -> None:
        """State-changing contract function `set_event_max_logs`.

        Args:
            caller: wire name `caller` (address).
            event_type: wire name `event_type` (symbol).
            new_max: wire name `new_max` (u32).
        """

        return self._transport.invoke("set_event_max_logs", [caller, event_type, new_max])

    def remove_event_cap(self, caller: str, event_type: str) -> None:
        """State-changing contract function `remove_event_cap`.

        Args:
            caller: wire name `caller` (address).
            event_type: wire name `event_type` (symbol).
        """

        return self._transport.invoke("remove_event_cap", [caller, event_type])

    def has_cap(self, event_type: str) -> bool:
        """Read-only contract function `has_cap`.

        Args:
            event_type: wire name `event_type` (symbol).
        Returns:
            bool.
        """

        return self._transport.invoke("has_cap", [event_type])

    def transfer_ownership(self, caller: str, new_owner: str) -> None:
        """State-changing contract function `transfer_ownership`.

        Args:
            caller: wire name `caller` (address).
            new_owner: wire name `new_owner` (address).
        """

        return self._transport.invoke("transfer_ownership", [caller, new_owner])

    def set_metadata_max_size(self, caller: str, max_size: int) -> None:
        """State-changing contract function `set_metadata_max_size`.

        Args:
            caller: wire name `caller` (address).
            max_size: wire name `max_size` (u32).
        """

        return self._transport.invoke("set_metadata_max_size", [caller, max_size])

    def set_event_metadata_max_size(self, caller: str, event_type: str, max_size: int) -> None:
        """State-changing contract function `set_event_metadata_max_size`.

        Args:
            caller: wire name `caller` (address).
            event_type: wire name `event_type` (symbol).
            max_size: wire name `max_size` (u32).
        """

        return self._transport.invoke("set_event_metadata_max_size", [caller, event_type, max_size])

    def set_metadata_schema(self, caller: str, event_type: str, schema: str) -> None:
        """State-changing contract function `set_metadata_schema`.

        Args:
            caller: wire name `caller` (address).
            event_type: wire name `event_type` (symbol).
            schema: wire name `schema` (bytes).
        """

        return self._transport.invoke("set_metadata_schema", [caller, event_type, schema])

    def get_metadata_schema(self, event_type: str) -> str:
        """Read-only contract function `get_metadata_schema`.

        Args:
            event_type: wire name `event_type` (symbol).
        Returns:
            bytes.
        """

        return self._transport.invoke("get_metadata_schema", [event_type])

    def register_schema(self, caller: str, event_type: str, schema: Schema, version: int) -> None:
        """State-changing contract function `register_schema`.

        Args:
            caller: wire name `caller` (address).
            event_type: wire name `event_type` (symbol).
            schema: wire name `schema` (Schema).
            version: wire name `version` (u32).
        """

        return self._transport.invoke("register_schema", [caller, event_type, schema, version])

    def get_schema(self, event_type: str, version: int) -> Optional[Schema]:
        """Read-only contract function `get_schema`.

        Args:
            event_type: wire name `event_type` (symbol).
            version: wire name `version` (u32).
        Returns:
            option<Schema>.
        """

        return self._transport.invoke("get_schema", [event_type, version])

    def list_schemas(self, event_type: str) -> List[int]:
        """Read-only contract function `list_schemas`.

        Args:
            event_type: wire name `event_type` (symbol).
        Returns:
            vec<u32>.
        """

        return self._transport.invoke("list_schemas", [event_type])

    def migrate_event_metadata(self, caller: str, event_type: str, from_version: int, to_version: int, migration_fn: MigrationFunction) -> None:
        """State-changing contract function `migrate_event_metadata`.

        Args:
            caller: wire name `caller` (address).
            event_type: wire name `event_type` (symbol).
            from_version: wire name `from_version` (u32).
            to_version: wire name `to_version` (u32).
            migration_fn: wire name `migration_fn` (MigrationFunction).
        """

        return self._transport.invoke("migrate_event_metadata", [caller, event_type, from_version, to_version, migration_fn])

    def get_migration_function(self, event_type: str, from_version: int, to_version: int) -> Optional[MigrationFunction]:
        """Read-only contract function `get_migration_function`.

        Args:
            event_type: wire name `event_type` (symbol).
            from_version: wire name `from_version` (u32).
            to_version: wire name `to_version` (u32).
        Returns:
            option<MigrationFunction>.
        """

        return self._transport.invoke("get_migration_function", [event_type, from_version, to_version])

    def check_schema_compatibility(self, left: Schema, right: Schema) -> SchemaCompatibility:
        """State-changing contract function `check_schema_compatibility`.

        Args:
            left: wire name `left` (Schema).
            right: wire name `right` (Schema).
        Returns:
            SchemaCompatibility.
        """

        return self._transport.invoke("check_schema_compatibility", [left, right])

    def is_backward_compatible(self, left: Schema, right: Schema) -> bool:
        """Read-only contract function `is_backward_compatible`.

        Args:
            left: wire name `left` (Schema).
            right: wire name `right` (Schema).
        Returns:
            bool.
        """

        return self._transport.invoke("is_backward_compatible", [left, right])

    def is_forward_compatible(self, left: Schema, right: Schema) -> bool:
        """Read-only contract function `is_forward_compatible`.

        Args:
            left: wire name `left` (Schema).
            right: wire name `right` (Schema).
        Returns:
            bool.
        """

        return self._transport.invoke("is_forward_compatible", [left, right])

    def set_event_ttl(self, caller: str, ttl_ledgers: int) -> None:
        """State-changing contract function `set_event_ttl`.

        Args:
            caller: wire name `caller` (address).
            ttl_ledgers: wire name `ttl_ledgers` (u32).
        """

        return self._transport.invoke("set_event_ttl", [caller, ttl_ledgers])

    def get_event_ttl(self) -> int:
        """Read-only contract function `get_event_ttl`.

        Returns:
            u32.
        """

        return self._transport.invoke("get_event_ttl", [])

    def cleanup_expired_events(self, caller: str, start_index: int, batch_size: int) -> int:
        """State-changing contract function `cleanup_expired_events`.

        Args:
            caller: wire name `caller` (address).
            start_index: wire name `start_index` (u32).
            batch_size: wire name `batch_size` (u32).
        Returns:
            u32.
        """

        return self._transport.invoke("cleanup_expired_events", [caller, start_index, batch_size])

    def get_cleanup_stats(self) -> TtlCleanupStats:
        """Read-only contract function `get_cleanup_stats`.

        Returns:
            TtlCleanupStats.
        """

        return self._transport.invoke("get_cleanup_stats", [])

    def register_webhook(self, caller: str, event_type: str, url: str, secret: str) -> None:
        """State-changing contract function `register_webhook`.

        Args:
            caller: wire name `caller` (address).
            event_type: wire name `event_type` (symbol).
            url: wire name `url` (bytes).
            secret: wire name `secret` (bytes).
        """

        return self._transport.invoke("register_webhook", [caller, event_type, url, secret])

    def unregister_webhook(self, caller: str, event_type: str, url: str) -> None:
        """State-changing contract function `unregister_webhook`.

        Args:
            caller: wire name `caller` (address).
            event_type: wire name `event_type` (symbol).
            url: wire name `url` (bytes).
        """

        return self._transport.invoke("unregister_webhook", [caller, event_type, url])

    def get_webhooks(self, event_type: str) -> List[str]:
        """Read-only contract function `get_webhooks`.

        Args:
            event_type: wire name `event_type` (symbol).
        Returns:
            vec<bytes>.
        """

        return self._transport.invoke("get_webhooks", [event_type])

    def pause(self, caller: str) -> None:
        """State-changing contract function `pause`.

        Args:
            caller: wire name `caller` (address).
        """

        return self._transport.invoke("pause", [caller])

    def unpause(self, caller: str) -> None:
        """State-changing contract function `unpause`.

        Args:
            caller: wire name `caller` (address).
        """

        return self._transport.invoke("unpause", [caller])

    def is_paused(self) -> bool:
        """Read-only contract function `is_paused`.

        Returns:
            bool.
        """

        return self._transport.invoke("is_paused", [])

    def paused_since(self) -> int:
        """State-changing contract function `paused_since`.

        Returns:
            u64.
        """

        return self._transport.invoke("paused_since", [])

    def set_category_max_len(self, caller: str, max_len: int) -> None:
        """State-changing contract function `set_category_max_len`.

        Args:
            caller: wire name `caller` (address).
            max_len: wire name `max_len` (u32).
        """

        return self._transport.invoke("set_category_max_len", [caller, max_len])

    def block_submitter(self, caller: str, submitter: str) -> None:
        """State-changing contract function `block_submitter`.

        Args:
            caller: wire name `caller` (address).
            submitter: wire name `submitter` (address).
        """

        return self._transport.invoke("block_submitter", [caller, submitter])

    def unblock_submitter(self, caller: str, submitter: str) -> None:
        """State-changing contract function `unblock_submitter`.

        Args:
            caller: wire name `caller` (address).
            submitter: wire name `submitter` (address).
        """

        return self._transport.invoke("unblock_submitter", [caller, submitter])

    def enable_allowlist_mode(self, caller: str) -> None:
        """State-changing contract function `enable_allowlist_mode`.

        Args:
            caller: wire name `caller` (address).
        """

        return self._transport.invoke("enable_allowlist_mode", [caller])

    def disable_allowlist_mode(self, caller: str) -> None:
        """State-changing contract function `disable_allowlist_mode`.

        Args:
            caller: wire name `caller` (address).
        """

        return self._transport.invoke("disable_allowlist_mode", [caller])

    def allow_submitter(self, caller: str, submitter: str) -> None:
        """State-changing contract function `allow_submitter`.

        Args:
            caller: wire name `caller` (address).
            submitter: wire name `submitter` (address).
        """

        return self._transport.invoke("allow_submitter", [caller, submitter])

    def remove_submitter_from_allowlist(self, caller: str, submitter: str) -> None:
        """State-changing contract function `remove_submitter_from_allowlist`.

        Args:
            caller: wire name `caller` (address).
            submitter: wire name `submitter` (address).
        """

        return self._transport.invoke("remove_submitter_from_allowlist", [caller, submitter])

    def get_metadata_max_size(self, event_type: str) -> int:
        """Read-only contract function `get_metadata_max_size`.

        Args:
            event_type: wire name `event_type` (symbol).
        Returns:
            u32.
        """

        return self._transport.invoke("get_metadata_max_size", [event_type])

    def get_statistics(self, caller: str) -> ContractStatistics:
        """Read-only contract function `get_statistics`.

        Args:
            caller: wire name `caller` (address).
        Returns:
            ContractStatistics.
        """

        return self._transport.invoke("get_statistics", [caller])

    def set_event_emission_mode(self, caller: str, mode: int) -> None:
        """State-changing contract function `set_event_emission_mode`.

        Args:
            caller: wire name `caller` (address).
            mode: wire name `mode` (u32).
        """

        return self._transport.invoke("set_event_emission_mode", [caller, mode])

    def get_event_emission_mode(self) -> int:
        """Read-only contract function `get_event_emission_mode`.

        Returns:
            u32.
        """

        return self._transport.invoke("get_event_emission_mode", [])

    def set_low_cost_mode(self, caller: str, enabled: bool) -> None:
        """State-changing contract function `set_low_cost_mode`.

        Args:
            caller: wire name `caller` (address).
            enabled: wire name `enabled` (bool).
        """

        return self._transport.invoke("set_low_cost_mode", [caller, enabled])

    def is_low_cost_mode(self) -> bool:
        """Read-only contract function `is_low_cost_mode`.

        Returns:
            bool.
        """

        return self._transport.invoke("is_low_cost_mode", [])

    def set_submitter_rate_limit(self, caller: str, submitter: str, max_per_timestamp: int) -> None:
        """State-changing contract function `set_submitter_rate_limit`.

        Args:
            caller: wire name `caller` (address).
            submitter: wire name `submitter` (address).
            max_per_timestamp: wire name `max_per_timestamp` (u32).
        """

        return self._transport.invoke("set_submitter_rate_limit", [caller, submitter, max_per_timestamp])

    def compact_storage(self, caller: str, stale_types: List[str]) -> int:
        """State-changing contract function `compact_storage`.

        Args:
            caller: wire name `caller` (address).
            stale_types: wire name `stale_types` (vec<symbol>).
        Returns:
            u32.
        """

        return self._transport.invoke("compact_storage", [caller, stale_types])

    def log_event_signed(self, submitter: str, event_type: str, metadata: str, signature_payload: str) -> str:
        """State-changing contract function `log_event_signed`.

        Args:
            submitter: wire name `submitter` (address).
            event_type: wire name `event_type` (symbol).
            metadata: wire name `metadata` (bytes).
            signature_payload: wire name `signature_payload` (bytes).
        Returns:
            bytesn<32>.
        """

        return self._transport.invoke("log_event_signed", [submitter, event_type, metadata, signature_payload])

    def get_event_signature(self, event_id: str) -> Optional[str]:
        """Read-only contract function `get_event_signature`.

        Args:
            event_id: wire name `event_id` (bytesn<32>).
        Returns:
            option<bytes>.
        """

        return self._transport.invoke("get_event_signature", [event_id])

    def find_event_by_content(self, event_type: str, submitter: str, metadata: str) -> Optional[Event]:
        """Read-only contract function `find_event_by_content`.

        Args:
            event_type: wire name `event_type` (symbol).
            submitter: wire name `submitter` (address).
            metadata: wire name `metadata` (bytes).
        Returns:
            option<Event>.
        """

        return self._transport.invoke("find_event_by_content", [event_type, submitter, metadata])

    def add_owner(self, caller: str, new_owner: str) -> None:
        """State-changing contract function `add_owner`.

        Args:
            caller: wire name `caller` (address).
            new_owner: wire name `new_owner` (address).
        """

        return self._transport.invoke("add_owner", [caller, new_owner])

    def remove_owner(self, caller: str, owner_to_remove: str) -> None:
        """State-changing contract function `remove_owner`.

        Args:
            caller: wire name `caller` (address).
            owner_to_remove: wire name `owner_to_remove` (address).
        """

        return self._transport.invoke("remove_owner", [caller, owner_to_remove])

    def set_required_signatures(self, caller: str, required: int) -> None:
        """State-changing contract function `set_required_signatures`.

        Args:
            caller: wire name `caller` (address).
            required: wire name `required` (u32).
        """

        return self._transport.invoke("set_required_signatures", [caller, required])

    def submit_proposal(self, proposer: str, action: ProposalAction, ttl_seconds: int) -> int:
        """State-changing contract function `submit_proposal`.

        Args:
            proposer: wire name `proposer` (address).
            action: wire name `action` (ProposalAction).
            ttl_seconds: wire name `ttl_seconds` (u64).
        Returns:
            u32.
        """

        return self._transport.invoke("submit_proposal", [proposer, action, ttl_seconds])

    def approve_proposal(self, approver: str, proposal_id: int) -> None:
        """State-changing contract function `approve_proposal`.

        Args:
            approver: wire name `approver` (address).
            proposal_id: wire name `proposal_id` (u32).
        """

        return self._transport.invoke("approve_proposal", [approver, proposal_id])

    def execute_proposal(self, executor: str, proposal_id: int) -> None:
        """State-changing contract function `execute_proposal`.

        Args:
            executor: wire name `executor` (address).
            proposal_id: wire name `proposal_id` (u32).
        """

        return self._transport.invoke("execute_proposal", [executor, proposal_id])

    def set_role(self, caller: str, target: str, role: Optional[Role]) -> None:
        """State-changing contract function `set_role`.

        Args:
            caller: wire name `caller` (address).
            target: wire name `target` (address).
            role: wire name `role` (option<Role>).
        """

        return self._transport.invoke("set_role", [caller, target, role])

    def get_role(self, target: str) -> Optional[Role]:
        """Read-only contract function `get_role`.

        Args:
            target: wire name `target` (address).
        Returns:
            option<Role>.
        """

        return self._transport.invoke("get_role", [target])

    def enable_rbac(self, caller: str, enabled: bool) -> None:
        """State-changing contract function `enable_rbac`.

        Args:
            caller: wire name `caller` (address).
            enabled: wire name `enabled` (bool).
        """

        return self._transport.invoke("enable_rbac", [caller, enabled])

    def is_rbac_enabled(self) -> bool:
        """Read-only contract function `is_rbac_enabled`.

        Returns:
            bool.
        """

        return self._transport.invoke("is_rbac_enabled", [])

    def set_dedup_policy(self, caller: str, policy: DedupPolicy) -> None:
        """State-changing contract function `set_dedup_policy`.

        Args:
            caller: wire name `caller` (address).
            policy: wire name `policy` (DedupPolicy).
        """

        return self._transport.invoke("set_dedup_policy", [caller, policy])

    def get_dedup_policy(self) -> DedupPolicy:
        """Read-only contract function `get_dedup_policy`.

        Returns:
            DedupPolicy.
        """

        return self._transport.invoke("get_dedup_policy", [])

    def set_dedup_policy_for_type(self, caller: str, event_type: str, policy: Optional[DedupPolicy]) -> None:
        """State-changing contract function `set_dedup_policy_for_type`.

        Args:
            caller: wire name `caller` (address).
            event_type: wire name `event_type` (symbol).
            policy: wire name `policy` (option<DedupPolicy>).
        """

        return self._transport.invoke("set_dedup_policy_for_type", [caller, event_type, policy])

    def get_dedup_policy_for_type(self, event_type: str) -> DedupPolicy:
        """Read-only contract function `get_dedup_policy_for_type`.

        Args:
            event_type: wire name `event_type` (symbol).
        Returns:
            DedupPolicy.
        """

        return self._transport.invoke("get_dedup_policy_for_type", [event_type])

    def log_event_with_custom_key(self, submitter: str, event_type: str, metadata: str, category: Optional[str], sub_event_type: Optional[str], force: bool, custom_key: Optional[str]) -> str:
        """State-changing contract function `log_event_with_custom_key`.

        Args:
            submitter: wire name `submitter` (address).
            event_type: wire name `event_type` (symbol).
            metadata: wire name `metadata` (bytes).
            category: wire name `category` (option<symbol>).
            sub_event_type: wire name `sub_event_type` (option<symbol>).
            force: wire name `force` (bool).
            custom_key: wire name `custom_key` (option<bytesn<32>>).
        Returns:
            bytesn<32>.
        """

        return self._transport.invoke("log_event_with_custom_key", [submitter, event_type, metadata, category, sub_event_type, force, custom_key])

    def cleanup_stale_dedup_entries(self, caller: str, start_index: int, batch_size: int) -> int:
        """State-changing contract function `cleanup_stale_dedup_entries`.

        Args:
            caller: wire name `caller` (address).
            start_index: wire name `start_index` (u32).
            batch_size: wire name `batch_size` (u32).
        Returns:
            u32.
        """

        return self._transport.invoke("cleanup_stale_dedup_entries", [caller, start_index, batch_size])

    def set_archive_config(self, caller: str, config: ArchiveConfig) -> None:
        """State-changing contract function `set_archive_config`.

        Args:
            caller: wire name `caller` (address).
            config: wire name `config` (ArchiveConfig).
        """

        return self._transport.invoke("set_archive_config", [caller, config])

    def get_archive_config(self) -> Optional[ArchiveConfig]:
        """Read-only contract function `get_archive_config`.

        Returns:
            option<ArchiveConfig>.
        """

        return self._transport.invoke("get_archive_config", [])

    def get_event_audit_trail(self, index: int) -> List[EventVersion]:
        """Read-only contract function `get_event_audit_trail`.

        Args:
            index: wire name `index` (u32).
        Returns:
            vec<EventVersion>.
        """

        return self._transport.invoke("get_event_audit_trail", [index])

    def tag_event_version(self, caller: str, index: int, version: int, tag: str) -> None:
        """State-changing contract function `tag_event_version`.

        Args:
            caller: wire name `caller` (address).
            index: wire name `index` (u32).
            version: wire name `version` (u32).
            tag: wire name `tag` (symbol).
        """

        return self._transport.invoke("tag_event_version", [caller, index, version, tag])

    def get_event_version_tag(self, index: int, version: int) -> Optional[str]:
        """Read-only contract function `get_event_version_tag`.

        Args:
            index: wire name `index` (u32).
            version: wire name `version` (u32).
        Returns:
            option<symbol>.
        """

        return self._transport.invoke("get_event_version_tag", [index, version])

    def get_event_diff(self, index: int, from_version: int, to_version: int) -> List[FieldChange]:
        """Read-only contract function `get_event_diff`.

        Args:
            index: wire name `index` (u32).
            from_version: wire name `from_version` (u32).
            to_version: wire name `to_version` (u32).
        Returns:
            vec<FieldChange>.
        """

        return self._transport.invoke("get_event_diff", [index, from_version, to_version])

    def compare_event_versions_detailed(self, index: int, from_version: int, to_version: int) -> VersionComparison:
        """State-changing contract function `compare_event_versions_detailed`.

        Args:
            index: wire name `index` (u32).
            from_version: wire name `from_version` (u32).
            to_version: wire name `to_version` (u32).
        Returns:
            VersionComparison.
        """

        return self._transport.invoke("compare_event_versions_detailed", [index, from_version, to_version])

    def as_dict(self) -> Union[dict, List[Any]]:
        """Return a small client summary, useful in debuggers and logs."""
        return {"contract_id": self.contract_id, "functions": len(self.FUNCTION_NAMES)}

# contract: AuditLedger v0.1.0
# idl: spec_version 1.0.0 (120 functions, 19 types, 75 errors, 43 events)