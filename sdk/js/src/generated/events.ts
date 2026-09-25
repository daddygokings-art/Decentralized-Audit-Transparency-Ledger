/**
 * GENERATED FILE — DO NOT EDIT.
 * Regenerate with: npm run generate --workspace tools/sdk-codegen (or `sdk-codegen generate`).
 * Source of truth: abi/audit-ledger.json
 *
 * Typed contract events. In Soroban the trailing topic is the event
 * discriminator; `topics` below lists the discriminator-relative topics.
 */


import type { Role } from './types';
export const ContractEventName = {
  AllowlistDisabled: 'allowlist_disabled',
  AllowlistEnabled: 'allowlist_enabled',
  ArchivedEventsPurged: 'archived_events_purged',
  ConfigSet: 'config_set',
  ContractPaused: 'contract_paused',
  ContractUnpaused: 'contract_unpaused',
  ContractUpgraded: 'contract_upgraded',
  DefaultNonceConfigSet: 'default_nonce_config_set',
  EventRolledBack: 'event_rolled_back',
  EventUpdated: 'event_updated',
  EventsArchived: 'events_archived',
  ExpiredRemoved: 'expired_removed',
  LogEvent: 'log_event',
  MigrateEventMetadata: 'migrate_event_metadata',
  NonceConfigSet: 'nonce_config_set',
  NonceReset: 'nonce_reset',
  OwnerAdded: 'owner_added',
  OwnerRemoved: 'owner_removed',
  PolicySet: 'policy_set',
  PolicySetType: 'policy_set_type',
  ProposalApproved: 'proposal_approved',
  ProposalExecuted: 'proposal_executed',
  ProposalSubmitted: 'proposal_submitted',
  RbacToggled: 'rbac_toggled',
  RegisterSchema: 'register_schema',
  RegisterWebhook: 'register_webhook',
  RemoveEventCap: 'remove_event_cap',
  RequiredSignaturesSet: 'required_signatures_set',
  RoleSet: 'role_set',
  SetEventTtl: 'set_event_ttl',
  SetGlobalMax: 'set_global_max',
  SetMetadataSchema: 'set_metadata_schema',
  SnapshotCreated: 'snapshot_created',
  StaleDedupCleaned: 'stale_dedup_cleaned',
  StaleHashesCleaned: 'stale_hashes_cleaned',
  StorageCompacted: 'storage_compacted',
  SubmitterAllowed: 'submitter_allowed',
  SubmitterBlocked: 'submitter_blocked',
  SubmitterRemovedFromAllowlist: 'submitter_removed_from_allowlist',
  SubmitterUnblocked: 'submitter_unblocked',
  TransferOwnership: 'transfer_ownership',
  UnregisterWebhook: 'unregister_webhook',
  VersionTagged: 'version_tagged',
} as const;

export type ContractEventNameLiteral = (typeof ContractEventName)[keyof typeof ContractEventName];

export const CONTRACT_EVENT_NAMES: readonly ContractEventNameLiteral[] = [
  'allowlist_disabled',
  'allowlist_enabled',
  'archived_events_purged',
  'config_set',
  'contract_paused',
  'contract_unpaused',
  'contract_upgraded',
  'default_nonce_config_set',
  'event_rolled_back',
  'event_updated',
  'events_archived',
  'expired_removed',
  'log_event',
  'migrate_event_metadata',
  'nonce_config_set',
  'nonce_reset',
  'owner_added',
  'owner_removed',
  'policy_set',
  'policy_set_type',
  'proposal_approved',
  'proposal_executed',
  'proposal_submitted',
  'rbac_toggled',
  'register_schema',
  'register_webhook',
  'remove_event_cap',
  'required_signatures_set',
  'role_set',
  'set_event_ttl',
  'set_global_max',
  'set_metadata_schema',
  'snapshot_created',
  'stale_dedup_cleaned',
  'stale_hashes_cleaned',
  'storage_compacted',
  'submitter_allowed',
  'submitter_blocked',
  'submitter_removed_from_allowlist',
  'submitter_unblocked',
  'transfer_ownership',
  'unregister_webhook',
  'version_tagged',
];

/**
 * `allowlist_disabled`
 * @data  1 address
 */
export interface AllowlistDisabledEvent {
  name: 'allowlist_disabled';
  data1: string;
}

/**
 * `allowlist_enabled`
 * @data  1 address
 */
export interface AllowlistEnabledEvent {
  name: 'allowlist_enabled';
  data1: string;
}

/**
 * `archived_events_purged`
 * @data  1 u32
 */
export interface ArchivedEventsPurgedEvent {
  name: 'archived_events_purged';
  data1: number;
}

/**
 * `config_set`
 * @topic 1 symbol
 * @data  1 address
 * @data  2 bool
 * @data  3 u32
 */
export interface ConfigSetEvent {
  name: 'config_set';
  topic1: string;
  data1: string;
  data2: boolean;
  data3: number;
}

/**
 * `contract_paused`
 * @data  1 address
 */
export interface ContractPausedEvent {
  name: 'contract_paused';
  data1: string;
}

/**
 * `contract_unpaused`
 * @data  1 address
 */
export interface ContractUnpausedEvent {
  name: 'contract_unpaused';
  data1: string;
}

/**
 * `contract_upgraded`
 * @data  1 option<bytesn<32>>
 * @data  2 bytesn<32>
 */
export interface ContractUpgradedEvent {
  name: 'contract_upgraded';
  data1: string | null;
  data2: string;
}

/**
 * `default_nonce_config_set`
 * @data  1 address
 * @data  2 u32
 * @data  3 u32
 */
export interface DefaultNonceConfigSetEvent {
  name: 'default_nonce_config_set';
  data1: string;
  data2: number;
  data3: number;
}

/**
 * `event_rolled_back`
 * @topic 1 symbol
 * @data  1 u32
 * @data  2 u32
 * @data  3 address
 * @data  4 u64
 */
export interface EventRolledBackEvent {
  name: 'event_rolled_back';
  topic1: string;
  data1: number;
  data2: number;
  data3: string;
  data4: number;
}

/**
 * `event_updated`
 * @data  1 u32
 * @data  2 bytesn<32>
 * @data  3 bytesn<32>
 * @data  4 address
 * @data  5 u64
 */
export interface EventUpdatedEvent {
  name: 'event_updated';
  data1: number;
  data2: string;
  data3: string;
  data4: string;
  data5: number;
}

/**
 * `events_archived`
 * @data  1 u32
 */
export interface EventsArchivedEvent {
  name: 'events_archived';
  data1: number;
}

/**
 * `expired_removed`
 * @topic 1 symbol
 * @data  1 address
 * @data  2 u32
 * @data  3 u32
 * @data  4 u32
 */
export interface ExpiredRemovedEvent {
  name: 'expired_removed';
  topic1: string;
  data1: string;
  data2: number;
  data3: number;
  data4: number;
}

/**
 * `log_event`
 * @topic 1 symbol
 * @data  1 address
 * @data  2 symbol
 * @data  3 u32
 */
export interface LogEventEvent {
  name: 'log_event';
  topic1: string;
  data1: string;
  data2: string;
  data3: number;
}

/**
 * `migrate_event_metadata`
 * @topic 1 symbol
 * @data  1 address
 * @data  2 symbol
 * @data  3 u32
 * @data  4 u32
 */
export interface MigrateEventMetadataEvent {
  name: 'migrate_event_metadata';
  topic1: string;
  data1: string;
  data2: string;
  data3: number;
  data4: number;
}

/**
 * `nonce_config_set`
 * @data  1 address
 * @data  2 u32
 * @data  3 u32
 */
export interface NonceConfigSetEvent {
  name: 'nonce_config_set';
  data1: string;
  data2: number;
  data3: number;
}

/**
 * `nonce_reset`
 * @data  1 address
 * @data  2 address
 */
export interface NonceResetEvent {
  name: 'nonce_reset';
  data1: string;
  data2: string;
}

/**
 * `owner_added`
 * @data  1 address
 */
export interface OwnerAddedEvent {
  name: 'owner_added';
  data1: string;
}

/**
 * `owner_removed`
 * @data  1 address
 */
export interface OwnerRemovedEvent {
  name: 'owner_removed';
  data1: string;
}

/**
 * `policy_set`
 * @topic 1 symbol
 * @data  1 address
 * @data  2 u32
 */
export interface PolicySetEvent {
  name: 'policy_set';
  topic1: string;
  data1: string;
  data2: number;
}

/**
 * `policy_set_type`
 * @topic 1 symbol
 * @data  1 address
 * @data  2 symbol
 * @data  3 option<u32>
 */
export interface PolicySetTypeEvent {
  name: 'policy_set_type';
  topic1: string;
  data1: string;
  data2: string;
  data3: number | null;
}

/**
 * `proposal_approved`
 * @data  1 u32
 * @data  2 address
 */
export interface ProposalApprovedEvent {
  name: 'proposal_approved';
  data1: number;
  data2: string;
}

/**
 * `proposal_executed`
 * @data  1 u32
 * @data  2 address
 */
export interface ProposalExecutedEvent {
  name: 'proposal_executed';
  data1: number;
  data2: string;
}

/**
 * `proposal_submitted`
 * @data  1 bytesn<32>
 */
export interface ProposalSubmittedEvent {
  name: 'proposal_submitted';
  data1: string;
}

/**
 * `rbac_toggled`
 * @topic 1 symbol
 * @data  1 address
 * @data  2 bool
 */
export interface RbacToggledEvent {
  name: 'rbac_toggled';
  topic1: string;
  data1: string;
  data2: boolean;
}

/**
 * `register_schema`
 * @topic 1 symbol
 * @data  1 address
 * @data  2 symbol
 * @data  3 u32
 */
export interface RegisterSchemaEvent {
  name: 'register_schema';
  topic1: string;
  data1: string;
  data2: string;
  data3: number;
}

/**
 * `register_webhook`
 * @topic 1 symbol
 * @data  1 address
 * @data  2 symbol
 * @data  3 bytes
 */
export interface RegisterWebhookEvent {
  name: 'register_webhook';
  topic1: string;
  data1: string;
  data2: string;
  data3: string;
}

/**
 * `remove_event_cap`
 * @topic 1 symbol
 * @data  1 address
 * @data  2 symbol
 */
export interface RemoveEventCapEvent {
  name: 'remove_event_cap';
  topic1: string;
  data1: string;
  data2: string;
}

/**
 * `required_signatures_set`
 * @data  1 u32
 */
export interface RequiredSignaturesSetEvent {
  name: 'required_signatures_set';
  data1: number;
}

/**
 * `role_set`
 * @topic 1 symbol
 * @data  1 address
 * @data  2 address
 * @data  3 option<Role>
 */
export interface RoleSetEvent {
  name: 'role_set';
  topic1: string;
  data1: string;
  data2: string;
  data3: Role | null;
}

/**
 * `set_event_ttl`
 * @topic 1 symbol
 * @data  1 address
 * @data  2 u32
 * @data  3 u32
 */
export interface SetEventTtlEvent {
  name: 'set_event_ttl';
  topic1: string;
  data1: string;
  data2: number;
  data3: number;
}

/**
 * `set_global_max`
 * @topic 1 symbol
 * @data  1 address
 * @data  2 u32
 * @data  3 u32
 */
export interface SetGlobalMaxEvent {
  name: 'set_global_max';
  topic1: string;
  data1: string;
  data2: number;
  data3: number;
}

/**
 * `set_metadata_schema`
 * @topic 1 symbol
 * @data  1 address
 * @data  2 symbol
 * @data  3 u32
 */
export interface SetMetadataSchemaEvent {
  name: 'set_metadata_schema';
  topic1: string;
  data1: string;
  data2: string;
  data3: number;
}

/**
 * `snapshot_created`
 * @data  1 u32
 * @data  2 u64
 * @data  3 u32
 */
export interface SnapshotCreatedEvent {
  name: 'snapshot_created';
  data1: number;
  data2: number;
  data3: number;
}

/**
 * `stale_dedup_cleaned`
 * @data  1 u32
 */
export interface StaleDedupCleanedEvent {
  name: 'stale_dedup_cleaned';
  data1: number;
}

/**
 * `stale_hashes_cleaned`
 * @data  1 u32
 */
export interface StaleHashesCleanedEvent {
  name: 'stale_hashes_cleaned';
  data1: number;
}

/**
 * `storage_compacted`
 * @data  1 u32
 */
export interface StorageCompactedEvent {
  name: 'storage_compacted';
  data1: number;
}

/**
 * `submitter_allowed`
 * @data  1 address
 * @data  2 address
 */
export interface SubmitterAllowedEvent {
  name: 'submitter_allowed';
  data1: string;
  data2: string;
}

/**
 * `submitter_blocked`
 * @data  1 address
 * @data  2 address
 */
export interface SubmitterBlockedEvent {
  name: 'submitter_blocked';
  data1: string;
  data2: string;
}

/**
 * `submitter_removed_from_allowlist`
 * @data  1 address
 * @data  2 address
 */
export interface SubmitterRemovedFromAllowlistEvent {
  name: 'submitter_removed_from_allowlist';
  data1: string;
  data2: string;
}

/**
 * `submitter_unblocked`
 * @data  1 address
 * @data  2 address
 */
export interface SubmitterUnblockedEvent {
  name: 'submitter_unblocked';
  data1: string;
  data2: string;
}

/**
 * `transfer_ownership`
 * @topic 1 symbol
 * @data  1 address
 * @data  2 address
 * @data  3 address
 */
export interface TransferOwnershipEvent {
  name: 'transfer_ownership';
  topic1: string;
  data1: string;
  data2: string;
  data3: string;
}

/**
 * `unregister_webhook`
 * @topic 1 symbol
 * @data  1 address
 * @data  2 symbol
 * @data  3 bytes
 */
export interface UnregisterWebhookEvent {
  name: 'unregister_webhook';
  topic1: string;
  data1: string;
  data2: string;
  data3: string;
}

/**
 * `version_tagged`
 * @topic 1 symbol
 * @data  1 address
 * @data  2 u32
 * @data  3 u32
 * @data  4 symbol
 */
export interface VersionTaggedEvent {
  name: 'version_tagged';
  topic1: string;
  data1: string;
  data2: number;
  data3: number;
  data4: string;
}

/** A decoded contract event whose payload has not been narrowed to a specific shape. */
export interface DecodedContractEvent {
  name: string;
  contractId: string;
  ledger: number;
  topics: string[];
  data: unknown[];
}

/**
 * contract: AuditLedger v0.1.0
 * idl: spec_version 1.0.0 (120 functions, 19 types, 75 errors, 43 events)
 */