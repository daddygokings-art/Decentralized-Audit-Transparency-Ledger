/**
 * GENERATED FILE — DO NOT EDIT.
 * Regenerate with: npm run generate --workspace tools/sdk-codegen (or `sdk-codegen generate`).
 * Source of truth: abi/audit-ledger.json
 *
 * Contract data types. Field names are preserved verbatim from the IDL so
 * that a value can be handed to the RPC layer without a translation step.
 */

/** Contract enum `SchemaFormat` (3 unit variant(s)). */
export type SchemaFormat = 'JsonSchemaDraft7' | 'JsonSchema201909' | 'Protobuf';

/** Wire ordinals for `SchemaFormat`, as encoded by the contract. */
export const SchemaFormatOrdinal: Record<SchemaFormat, number> = {
  JsonSchemaDraft7: 0,
  JsonSchema201909: 1,
  Protobuf: 2,
};

/** Every `SchemaFormat` value, in wire order. */
export const SchemaFormatValues: readonly SchemaFormat[] = [
  'JsonSchemaDraft7',
  'JsonSchema201909',
  'Protobuf',
];

/** Decode a wire ordinal into a `SchemaFormat`; throws on unknown ordinals. */
export function SchemaFormatFromOrdinal(value: number): SchemaFormat {
  const found = SchemaFormatValues.find((candidate) => SchemaFormatOrdinal[candidate] === value);
  if (found === undefined) {
    throw new Error(`Unknown SchemaFormat ordinal: ${value}`);
  }
  return found;
}

/** Contract enum `SchemaCompatibility` (5 unit variant(s)). */
export type SchemaCompatibility = 'Full' | 'Backward' | 'Forward' | 'Breaking' | 'Unknown';

/** Wire ordinals for `SchemaCompatibility`, as encoded by the contract. */
export const SchemaCompatibilityOrdinal: Record<SchemaCompatibility, number> = {
  Full: 0,
  Backward: 1,
  Forward: 2,
  Breaking: 3,
  Unknown: 4,
};

/** Every `SchemaCompatibility` value, in wire order. */
export const SchemaCompatibilityValues: readonly SchemaCompatibility[] = [
  'Full',
  'Backward',
  'Forward',
  'Breaking',
  'Unknown',
];

/** Decode a wire ordinal into a `SchemaCompatibility`; throws on unknown ordinals. */
export function SchemaCompatibilityFromOrdinal(value: number): SchemaCompatibility {
  const found = SchemaCompatibilityValues.find((candidate) => SchemaCompatibilityOrdinal[candidate] === value);
  if (found === undefined) {
    throw new Error(`Unknown SchemaCompatibility ordinal: ${value}`);
  }
  return found;
}

/** Contract type `Schema` (4 field(s)). */
export interface Schema {
  format: SchemaFormat;
  version: number;
  definition: string;
  compatibility: SchemaCompatibility;
}

/** Contract type `MigrationFunction` (4 field(s)). */
export interface MigrationFunction {
  from_version: number;
  to_version: number;
  name: string;
  body: string;
}

/** Contract type `Event` (11 field(s)). */
export interface Event {
  index: number;
  timestamp: number;
  event_type: string;
  category: string;
  submitter: string;
  metadata: string;
  sub_event_type: string | null;
  version: number;
  event_hash: string;
  prev_hash: string;
  parent_event_id: string | null;
}

/** Contract type `EventHeader` (4 field(s)). */
export interface EventHeader {
  index: number;
  timestamp: number;
  event_type: string;
  submitter: string;
}

/** Contract type `EventVersion` (4 field(s)). */
export interface EventVersion {
  version: number;
  data: Event;
  updated_at: number;
  updated_by: string;
}

/** Contract type `NonceState` (3 field(s)). */
export interface NonceState {
  last_nonce: number;
  window_size: number;
  max_nonce: number;
}

/** Contract type `ContractStatistics` (6 field(s)). */
export interface ContractStatistics {
  total_events: number;
  events_by_type: Array<[string, number]>;
  events_last_hour: number;
  events_last_day: number;
  events_last_week: number;
  top_submitters: Array<[string, number]>;
}

/** Contract enum `ProposalAction` (9 payload variant(s)). */
export type ProposalAction =
  | { variant: 'TransferOwnership'; ordinal: 0; value: { field_0: string } }
  | { variant: 'AddOwner'; ordinal: 1; value: { field_0: string } }
  | { variant: 'RemoveOwner'; ordinal: 2; value: { field_0: string } }
  | { variant: 'SetRequiredSignatures'; ordinal: 3; value: { field_0: number } }
  | { variant: 'SetGlobalMaxLogs'; ordinal: 4; value: { field_0: number } }
  | { variant: 'SetMetadataSchema'; ordinal: 5; value: { field_0: string; field_1: string } }
  | { variant: 'RollbackEvent'; ordinal: 6; value: { field_0: number; field_1: number } }
  | { variant: 'Pause'; ordinal: 7 }
  | { variant: 'Unpause'; ordinal: 8 };

/** Variant names carried by `ProposalAction`, in wire order. */
export const ProposalActionVariants: readonly ProposalAction['variant'][] = [
  'TransferOwnership',
  'AddOwner',
  'RemoveOwner',
  'SetRequiredSignatures',
  'SetGlobalMaxLogs',
  'SetMetadataSchema',
  'RollbackEvent',
  'Pause',
  'Unpause',
];

/** Constructors for every `ProposalAction` variant. */
export const ProposalAction = {
  TransferOwnership(field_0: string) {
    return { variant: 'TransferOwnership', ordinal: 0, value: { field_0 } };
  },
  AddOwner(field_0: string) {
    return { variant: 'AddOwner', ordinal: 1, value: { field_0 } };
  },
  RemoveOwner(field_0: string) {
    return { variant: 'RemoveOwner', ordinal: 2, value: { field_0 } };
  },
  SetRequiredSignatures(field_0: number) {
    return { variant: 'SetRequiredSignatures', ordinal: 3, value: { field_0 } };
  },
  SetGlobalMaxLogs(field_0: number) {
    return { variant: 'SetGlobalMaxLogs', ordinal: 4, value: { field_0 } };
  },
  SetMetadataSchema(field_0: string, field_1: string) {
    return { variant: 'SetMetadataSchema', ordinal: 5, value: { field_0, field_1 } };
  },
  RollbackEvent(field_0: number, field_1: number) {
    return { variant: 'RollbackEvent', ordinal: 6, value: { field_0, field_1 } };
  },
  Pause() {
    return { variant: 'Pause', ordinal: 7 };
  },
  Unpause() {
    return { variant: 'Unpause', ordinal: 8 };
  },
} as const;

/** Contract type `Snapshot` (5 field(s)). */
export interface Snapshot {
  id: number;
  timestamp: number;
  event_count: number;
  event_hash: string;
  description: string;
}

/** Contract type `TtlCleanupStats` (4 field(s)). */
export interface TtlCleanupStats {
  runs: number;
  ttl_extensions: number;
  cleaned: number;
  last_run_ledger: number;
}

/** Contract enum `Role` (4 unit variant(s)). */
export type Role = 'Admin' | 'Auditor' | 'Submitter' | 'Viewer';

/** Wire ordinals for `Role`, as encoded by the contract. */
export const RoleOrdinal: Record<Role, number> = {
  Admin: 0,
  Auditor: 1,
  Submitter: 2,
  Viewer: 3,
};

/** Every `Role` value, in wire order. */
export const RoleValues: readonly Role[] = [
  'Admin',
  'Auditor',
  'Submitter',
  'Viewer',
];

/** Decode a wire ordinal into a `Role`; throws on unknown ordinals. */
export function RoleFromOrdinal(value: number): Role {
  const found = RoleValues.find((candidate) => RoleOrdinal[candidate] === value);
  if (found === undefined) {
    throw new Error(`Unknown Role ordinal: ${value}`);
  }
  return found;
}

/** Contract enum `DedupPolicy` (4 unit variant(s)). */
export type DedupPolicy = 'None' | 'ContentHash' | 'ContentHashWithTimestamp' | 'Custom';

/** Wire ordinals for `DedupPolicy`, as encoded by the contract. */
export const DedupPolicyOrdinal: Record<DedupPolicy, number> = {
  None: 0,
  ContentHash: 1,
  ContentHashWithTimestamp: 2,
  Custom: 3,
};

/** Every `DedupPolicy` value, in wire order. */
export const DedupPolicyValues: readonly DedupPolicy[] = [
  'None',
  'ContentHash',
  'ContentHashWithTimestamp',
  'Custom',
];

/** Decode a wire ordinal into a `DedupPolicy`; throws on unknown ordinals. */
export function DedupPolicyFromOrdinal(value: number): DedupPolicy {
  const found = DedupPolicyValues.find((candidate) => DedupPolicyOrdinal[candidate] === value);
  if (found === undefined) {
    throw new Error(`Unknown DedupPolicy ordinal: ${value}`);
  }
  return found;
}

/** Contract type `ArchiveConfig` (3 field(s)). */
export interface ArchiveConfig {
  offchain_storage: boolean;
  base_url: string;
  compression: number;
}

/** Contract type `ArchivedEventRef` (5 field(s)). */
export interface ArchivedEventRef {
  id: string;
  index: number;
  checksum: string;
  url: string;
  archived_at: number;
}

/** Contract type `ArchiveStats` (3 field(s)). */
export interface ArchiveStats {
  total_archived: number;
  total_compressed: number;
  total_offchain: number;
}

/** Contract type `FieldChange` (3 field(s)). */
export interface FieldChange {
  field: string;
  from: string;
  to: string;
}

/** Contract type `VersionComparison` (7 field(s)). */
export interface VersionComparison {
  index: number;
  from_version: number;
  to_version: number;
  same: boolean;
  changes: Array<FieldChange>;
  from_hash: string;
  to_hash: string;
}

/**
 * contract: AuditLedger v0.1.0
 * idl: spec_version 1.0.0 (120 functions, 19 types, 75 errors, 43 events)
 */
