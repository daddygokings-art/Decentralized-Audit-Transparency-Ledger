# GENERATED FILE — DO NOT EDIT.
# Regenerate with: npm run generate --workspace tools/sdk-codegen (or `sdk-codegen generate`).
# Source of truth: abi/audit-ledger.json
"""

Typed contract events. In Soroban the trailing topic is the event
discriminator; `topics` below lists the discriminator-relative topics.
"""

from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional, Sequence, Tuple
from .types import Role

__all__ = [
    "ContractEvent",
    "CONTRACT_EVENT_NAMES",
    "EVENT_PAYLOAD_SHAPES",
    "AllowlistDisabledEvent",
    "AllowlistEnabledEvent",
    "ArchivedEventsPurgedEvent",
    "ConfigSetEvent",
    "ContractPausedEvent",
    "ContractUnpausedEvent",
    "ContractUpgradedEvent",
    "DefaultNonceConfigSetEvent",
    "EventRolledBackEvent",
    "EventUpdatedEvent",
    "EventsArchivedEvent",
    "ExpiredRemovedEvent",
    "LogEventEvent",
    "MigrateEventMetadataEvent",
    "NonceConfigSetEvent",
    "NonceResetEvent",
    "OwnerAddedEvent",
    "OwnerRemovedEvent",
    "PolicySetEvent",
    "PolicySetTypeEvent",
    "ProposalApprovedEvent",
    "ProposalExecutedEvent",
    "ProposalSubmittedEvent",
    "RbacToggledEvent",
    "RegisterSchemaEvent",
    "RegisterWebhookEvent",
    "RemoveEventCapEvent",
    "RequiredSignaturesSetEvent",
    "RoleSetEvent",
    "SetEventTtlEvent",
    "SetGlobalMaxEvent",
    "SetMetadataSchemaEvent",
    "SnapshotCreatedEvent",
    "StaleDedupCleanedEvent",
    "StaleHashesCleanedEvent",
    "StorageCompactedEvent",
    "SubmitterAllowedEvent",
    "SubmitterBlockedEvent",
    "SubmitterRemovedFromAllowlistEvent",
    "SubmitterUnblockedEvent",
    "TransferOwnershipEvent",
    "UnregisterWebhookEvent",
    "VersionTaggedEvent",
]

CONTRACT_EVENT_NAMES: Tuple[str, ...] = (
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
)


@dataclass(frozen=True)
class ContractEvent:
    """A decoded contract event, before payload narrowing."""

    name: str
    contract_id: str
    ledger: int
    topics: Sequence[str] = field(default_factory=tuple)
    data: Sequence[Any] = field(default_factory=tuple)


@dataclass(frozen=True)
class AllowlistDisabledEvent:
    """Decoded `allowlist_disabled` contract event."""

    name: str = "allowlist_disabled"
    contract_id: str = ""
    ledger: int = 0
    data1: str = ""


@dataclass(frozen=True)
class AllowlistEnabledEvent:
    """Decoded `allowlist_enabled` contract event."""

    name: str = "allowlist_enabled"
    contract_id: str = ""
    ledger: int = 0
    data1: str = ""


@dataclass(frozen=True)
class ArchivedEventsPurgedEvent:
    """Decoded `archived_events_purged` contract event."""

    name: str = "archived_events_purged"
    contract_id: str = ""
    ledger: int = 0
    data1: int = 0


@dataclass(frozen=True)
class ConfigSetEvent:
    """Decoded `config_set` contract event."""

    name: str = "config_set"
    contract_id: str = ""
    ledger: int = 0
    topic1: str = ""
    data1: str = ""
    data2: bool = False
    data3: int = 0


@dataclass(frozen=True)
class ContractPausedEvent:
    """Decoded `contract_paused` contract event."""

    name: str = "contract_paused"
    contract_id: str = ""
    ledger: int = 0
    data1: str = ""


@dataclass(frozen=True)
class ContractUnpausedEvent:
    """Decoded `contract_unpaused` contract event."""

    name: str = "contract_unpaused"
    contract_id: str = ""
    ledger: int = 0
    data1: str = ""


@dataclass(frozen=True)
class ContractUpgradedEvent:
    """Decoded `contract_upgraded` contract event."""

    name: str = "contract_upgraded"
    contract_id: str = ""
    ledger: int = 0
    data1: Optional[str] = None
    data2: str = ""


@dataclass(frozen=True)
class DefaultNonceConfigSetEvent:
    """Decoded `default_nonce_config_set` contract event."""

    name: str = "default_nonce_config_set"
    contract_id: str = ""
    ledger: int = 0
    data1: str = ""
    data2: int = 0
    data3: int = 0


@dataclass(frozen=True)
class EventRolledBackEvent:
    """Decoded `event_rolled_back` contract event."""

    name: str = "event_rolled_back"
    contract_id: str = ""
    ledger: int = 0
    topic1: str = ""
    data1: int = 0
    data2: int = 0
    data3: str = ""
    data4: int = 0


@dataclass(frozen=True)
class EventUpdatedEvent:
    """Decoded `event_updated` contract event."""

    name: str = "event_updated"
    contract_id: str = ""
    ledger: int = 0
    data1: int = 0
    data2: str = ""
    data3: str = ""
    data4: str = ""
    data5: int = 0


@dataclass(frozen=True)
class EventsArchivedEvent:
    """Decoded `events_archived` contract event."""

    name: str = "events_archived"
    contract_id: str = ""
    ledger: int = 0
    data1: int = 0


@dataclass(frozen=True)
class ExpiredRemovedEvent:
    """Decoded `expired_removed` contract event."""

    name: str = "expired_removed"
    contract_id: str = ""
    ledger: int = 0
    topic1: str = ""
    data1: str = ""
    data2: int = 0
    data3: int = 0
    data4: int = 0


@dataclass(frozen=True)
class LogEventEvent:
    """Decoded `log_event` contract event."""

    name: str = "log_event"
    contract_id: str = ""
    ledger: int = 0
    topic1: str = ""
    data1: str = ""
    data2: str = ""
    data3: int = 0


@dataclass(frozen=True)
class MigrateEventMetadataEvent:
    """Decoded `migrate_event_metadata` contract event."""

    name: str = "migrate_event_metadata"
    contract_id: str = ""
    ledger: int = 0
    topic1: str = ""
    data1: str = ""
    data2: str = ""
    data3: int = 0
    data4: int = 0


@dataclass(frozen=True)
class NonceConfigSetEvent:
    """Decoded `nonce_config_set` contract event."""

    name: str = "nonce_config_set"
    contract_id: str = ""
    ledger: int = 0
    data1: str = ""
    data2: int = 0
    data3: int = 0


@dataclass(frozen=True)
class NonceResetEvent:
    """Decoded `nonce_reset` contract event."""

    name: str = "nonce_reset"
    contract_id: str = ""
    ledger: int = 0
    data1: str = ""
    data2: str = ""


@dataclass(frozen=True)
class OwnerAddedEvent:
    """Decoded `owner_added` contract event."""

    name: str = "owner_added"
    contract_id: str = ""
    ledger: int = 0
    data1: str = ""


@dataclass(frozen=True)
class OwnerRemovedEvent:
    """Decoded `owner_removed` contract event."""

    name: str = "owner_removed"
    contract_id: str = ""
    ledger: int = 0
    data1: str = ""


@dataclass(frozen=True)
class PolicySetEvent:
    """Decoded `policy_set` contract event."""

    name: str = "policy_set"
    contract_id: str = ""
    ledger: int = 0
    topic1: str = ""
    data1: str = ""
    data2: int = 0


@dataclass(frozen=True)
class PolicySetTypeEvent:
    """Decoded `policy_set_type` contract event."""

    name: str = "policy_set_type"
    contract_id: str = ""
    ledger: int = 0
    topic1: str = ""
    data1: str = ""
    data2: str = ""
    data3: Optional[int] = None


@dataclass(frozen=True)
class ProposalApprovedEvent:
    """Decoded `proposal_approved` contract event."""

    name: str = "proposal_approved"
    contract_id: str = ""
    ledger: int = 0
    data1: int = 0
    data2: str = ""


@dataclass(frozen=True)
class ProposalExecutedEvent:
    """Decoded `proposal_executed` contract event."""

    name: str = "proposal_executed"
    contract_id: str = ""
    ledger: int = 0
    data1: int = 0
    data2: str = ""


@dataclass(frozen=True)
class ProposalSubmittedEvent:
    """Decoded `proposal_submitted` contract event."""

    name: str = "proposal_submitted"
    contract_id: str = ""
    ledger: int = 0
    data1: str = ""


@dataclass(frozen=True)
class RbacToggledEvent:
    """Decoded `rbac_toggled` contract event."""

    name: str = "rbac_toggled"
    contract_id: str = ""
    ledger: int = 0
    topic1: str = ""
    data1: str = ""
    data2: bool = False


@dataclass(frozen=True)
class RegisterSchemaEvent:
    """Decoded `register_schema` contract event."""

    name: str = "register_schema"
    contract_id: str = ""
    ledger: int = 0
    topic1: str = ""
    data1: str = ""
    data2: str = ""
    data3: int = 0


@dataclass(frozen=True)
class RegisterWebhookEvent:
    """Decoded `register_webhook` contract event."""

    name: str = "register_webhook"
    contract_id: str = ""
    ledger: int = 0
    topic1: str = ""
    data1: str = ""
    data2: str = ""
    data3: str = ""


@dataclass(frozen=True)
class RemoveEventCapEvent:
    """Decoded `remove_event_cap` contract event."""

    name: str = "remove_event_cap"
    contract_id: str = ""
    ledger: int = 0
    topic1: str = ""
    data1: str = ""
    data2: str = ""


@dataclass(frozen=True)
class RequiredSignaturesSetEvent:
    """Decoded `required_signatures_set` contract event."""

    name: str = "required_signatures_set"
    contract_id: str = ""
    ledger: int = 0
    data1: int = 0


@dataclass(frozen=True)
class RoleSetEvent:
    """Decoded `role_set` contract event."""

    name: str = "role_set"
    contract_id: str = ""
    ledger: int = 0
    topic1: str = ""
    data1: str = ""
    data2: str = ""
    data3: Optional[Role] = None


@dataclass(frozen=True)
class SetEventTtlEvent:
    """Decoded `set_event_ttl` contract event."""

    name: str = "set_event_ttl"
    contract_id: str = ""
    ledger: int = 0
    topic1: str = ""
    data1: str = ""
    data2: int = 0
    data3: int = 0


@dataclass(frozen=True)
class SetGlobalMaxEvent:
    """Decoded `set_global_max` contract event."""

    name: str = "set_global_max"
    contract_id: str = ""
    ledger: int = 0
    topic1: str = ""
    data1: str = ""
    data2: int = 0
    data3: int = 0


@dataclass(frozen=True)
class SetMetadataSchemaEvent:
    """Decoded `set_metadata_schema` contract event."""

    name: str = "set_metadata_schema"
    contract_id: str = ""
    ledger: int = 0
    topic1: str = ""
    data1: str = ""
    data2: str = ""
    data3: int = 0


@dataclass(frozen=True)
class SnapshotCreatedEvent:
    """Decoded `snapshot_created` contract event."""

    name: str = "snapshot_created"
    contract_id: str = ""
    ledger: int = 0
    data1: int = 0
    data2: int = 0
    data3: int = 0


@dataclass(frozen=True)
class StaleDedupCleanedEvent:
    """Decoded `stale_dedup_cleaned` contract event."""

    name: str = "stale_dedup_cleaned"
    contract_id: str = ""
    ledger: int = 0
    data1: int = 0


@dataclass(frozen=True)
class StaleHashesCleanedEvent:
    """Decoded `stale_hashes_cleaned` contract event."""

    name: str = "stale_hashes_cleaned"
    contract_id: str = ""
    ledger: int = 0
    data1: int = 0


@dataclass(frozen=True)
class StorageCompactedEvent:
    """Decoded `storage_compacted` contract event."""

    name: str = "storage_compacted"
    contract_id: str = ""
    ledger: int = 0
    data1: int = 0


@dataclass(frozen=True)
class SubmitterAllowedEvent:
    """Decoded `submitter_allowed` contract event."""

    name: str = "submitter_allowed"
    contract_id: str = ""
    ledger: int = 0
    data1: str = ""
    data2: str = ""


@dataclass(frozen=True)
class SubmitterBlockedEvent:
    """Decoded `submitter_blocked` contract event."""

    name: str = "submitter_blocked"
    contract_id: str = ""
    ledger: int = 0
    data1: str = ""
    data2: str = ""


@dataclass(frozen=True)
class SubmitterRemovedFromAllowlistEvent:
    """Decoded `submitter_removed_from_allowlist` contract event."""

    name: str = "submitter_removed_from_allowlist"
    contract_id: str = ""
    ledger: int = 0
    data1: str = ""
    data2: str = ""


@dataclass(frozen=True)
class SubmitterUnblockedEvent:
    """Decoded `submitter_unblocked` contract event."""

    name: str = "submitter_unblocked"
    contract_id: str = ""
    ledger: int = 0
    data1: str = ""
    data2: str = ""


@dataclass(frozen=True)
class TransferOwnershipEvent:
    """Decoded `transfer_ownership` contract event."""

    name: str = "transfer_ownership"
    contract_id: str = ""
    ledger: int = 0
    topic1: str = ""
    data1: str = ""
    data2: str = ""
    data3: str = ""


@dataclass(frozen=True)
class UnregisterWebhookEvent:
    """Decoded `unregister_webhook` contract event."""

    name: str = "unregister_webhook"
    contract_id: str = ""
    ledger: int = 0
    topic1: str = ""
    data1: str = ""
    data2: str = ""
    data3: str = ""


@dataclass(frozen=True)
class VersionTaggedEvent:
    """Decoded `version_tagged` contract event."""

    name: str = "version_tagged"
    contract_id: str = ""
    ledger: int = 0
    topic1: str = ""
    data1: str = ""
    data2: int = 0
    data3: int = 0
    data4: str = ""


#: Event name -> ordered (topic types, data types) as encoded on chain.
EVENT_PAYLOAD_SHAPES: Dict[str, Tuple[List[str], List[str]]] = {
    "allowlist_disabled": ([], ["address"]),
    "allowlist_enabled": ([], ["address"]),
    "archived_events_purged": ([], ["u32"]),
    "config_set": (["symbol"], ["address", "bool", "u32"]),
    "contract_paused": ([], ["address"]),
    "contract_unpaused": ([], ["address"]),
    "contract_upgraded": ([], ["option<bytesn<32>>", "bytesn<32>"]),
    "default_nonce_config_set": ([], ["address", "u32", "u32"]),
    "event_rolled_back": (["symbol"], ["u32", "u32", "address", "u64"]),
    "event_updated": ([], ["u32", "bytesn<32>", "bytesn<32>", "address", "u64"]),
    "events_archived": ([], ["u32"]),
    "expired_removed": (["symbol"], ["address", "u32", "u32", "u32"]),
    "log_event": (["symbol"], ["address", "symbol", "u32"]),
    "migrate_event_metadata": (["symbol"], ["address", "symbol", "u32", "u32"]),
    "nonce_config_set": ([], ["address", "u32", "u32"]),
    "nonce_reset": ([], ["address", "address"]),
    "owner_added": ([], ["address"]),
    "owner_removed": ([], ["address"]),
    "policy_set": (["symbol"], ["address", "u32"]),
    "policy_set_type": (["symbol"], ["address", "symbol", "option<u32>"]),
    "proposal_approved": ([], ["u32", "address"]),
    "proposal_executed": ([], ["u32", "address"]),
    "proposal_submitted": ([], ["bytesn<32>"]),
    "rbac_toggled": (["symbol"], ["address", "bool"]),
    "register_schema": (["symbol"], ["address", "symbol", "u32"]),
    "register_webhook": (["symbol"], ["address", "symbol", "bytes"]),
    "remove_event_cap": (["symbol"], ["address", "symbol"]),
    "required_signatures_set": ([], ["u32"]),
    "role_set": (["symbol"], ["address", "address", "option<Role>"]),
    "set_event_ttl": (["symbol"], ["address", "u32", "u32"]),
    "set_global_max": (["symbol"], ["address", "u32", "u32"]),
    "set_metadata_schema": (["symbol"], ["address", "symbol", "u32"]),
    "snapshot_created": ([], ["u32", "u64", "u32"]),
    "stale_dedup_cleaned": ([], ["u32"]),
    "stale_hashes_cleaned": ([], ["u32"]),
    "storage_compacted": ([], ["u32"]),
    "submitter_allowed": ([], ["address", "address"]),
    "submitter_blocked": ([], ["address", "address"]),
    "submitter_removed_from_allowlist": ([], ["address", "address"]),
    "submitter_unblocked": ([], ["address", "address"]),
    "transfer_ownership": (["symbol"], ["address", "address", "address"]),
    "unregister_webhook": (["symbol"], ["address", "symbol", "bytes"]),
    "version_tagged": (["symbol"], ["address", "u32", "u32", "symbol"]),
}

# contract: AuditLedger v0.1.0
# idl: spec_version 1.0.0 (120 functions, 19 types, 75 errors, 43 events)