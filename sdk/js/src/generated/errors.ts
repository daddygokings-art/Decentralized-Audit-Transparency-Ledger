/**
 * GENERATED FILE — DO NOT EDIT.
 * Regenerate with: npm run generate --workspace tools/sdk-codegen (or `sdk-codegen generate`).
 * Source of truth: abi/audit-ledger.json
 *
 * Contract error codes. Codes are stable wire values; never renumber them.
 */

export const ContractErrorCode = {
  CallerNotOwner: 1,
  GlobalMaxLogsReached: 2,
  EventTypeMaxLogsReached: 3,
  EventDoesNotExist: 4,
  EventTypeIndexOutOfBounds: 5,
  NewOwnerIsZero: 6,
  CapNotSet: 7,
  MetadataTooLarge: 8,
  ContractNotInitialized: 9,
  TotalEventsOverflow: 10,
  TimestampOutOfRange: 11,
  InvalidSignature: 12,
  ContractPaused: 13,
  RateLimitExceeded: 14,
  SameOwner: 15,
  MaxLogsBelowCurrentCount: 16,
  CapAlreadyRemoved: 17,
  CapNeverSet: 18,
  NonceTooLow: 19,
  NoEventsForType: 20,
  InvalidPaginationParams: 21,
  InvalidWasmHash: 22,
  SubmitterBlocked: 23,
  CategoryTooLong: 24,
  ReentrancyDetected: 25,
  AlreadyInitialized: 26,
  NonceExhausted: 27,
  NonceWindowExceeded: 28,
  NonceResetNotExhausted: 29,
  SnapshotNotFound: 30,
  SnapshotVerificationFailed: 31,
  MetadataSchemaViolation: 32,
  InvalidVersion: 33,
  MaterialPassportAlreadyExists: 34,
  MaterialPassportNotFound: 35,
  InvalidLoopEventType: 36,
  InvalidFlowQuantity: 37,
  LcaProfileAlreadyExists: 38,
  LcaProfileNotFound: 39,
  InvalidLcaPhase: 40,
  InvalidImpactCategory: 41,
  LcaAlreadyFinalized: 42,
  LcaNotFinalized: 43,
  LcaNormRefNotFound: 44,
  LcaWeightingSchemeNotFound: 45,
  LcaDbEntryNotFound: 46,
  BioImpactNotFound: 47,
  BioOffsetNotFound: 48,
  InvalidLandUseType: 49,
  InvalidEcoServiceCat: 50,
  InvalidLandArea: 51,
  InvalidOffsetQuantity: 52,
  OffsetAlreadyRetired: 53,
  OffsetRetirementExceedsBalance: 54,
  SpeciesObservationNotFound: 55,
  WaterFootprintNotFound: 56,
  WaterRiskNotFound: 57,
  WaterStewardshipNotFound: 58,
  WaterDisclosureNotFound: 59,
  InvalidWaterSector: 60,
  InvalidWaterVolume: 61,
  InvalidScarcityFactor: 62,
  InvalidDisclosureYear: 63,
  EventOnLegalHold: 64,
  LegalHoldNotFound: 65,
  ComplianceExceptionActive: 66,
  EventAlreadyErased: 67,
  ErasureRequestNotFound: 68,
  ErasureRequestAlreadyDecided: 69,
  InvalidRetentionPeriod: 70,
  EmptyComplianceReason: 71,
  OperationalEventNotErasable: 72,
  UnauthorizedOpsRecorder: 73,
  RoleNotGranted: 74,
  InvalidDedupPolicy: 75,
} as const;

export type ContractErrorName = keyof typeof ContractErrorCode;

/** Numeric code for a contract error, or `undefined` when unknown. */
export function contractErrorCode(name: string): number | undefined {
  return (ContractErrorCode as Record<string, number>)[name];
}

/** Canonical error name for a numeric code, or `undefined` when unknown. */
export function contractErrorName(code: number): ContractErrorName | undefined {
  return (Object.keys(ContractErrorCode) as ContractErrorName[]).find(
    (name) => ContractErrorCode[name] === code,
  );
}

/** Human-readable message for a contract error code. */
export function describeContractError(code: number): string {
  const name = contractErrorName(code);
  return name === undefined ? `Unknown contract error code ${code}` : `${name} (${code})`;
}

/** All contract error names, ascending by wire code. */
export const CONTRACT_ERROR_NAMES: readonly ContractErrorName[] = [
  'CallerNotOwner',
  'GlobalMaxLogsReached',
  'EventTypeMaxLogsReached',
  'EventDoesNotExist',
  'EventTypeIndexOutOfBounds',
  'NewOwnerIsZero',
  'CapNotSet',
  'MetadataTooLarge',
  'ContractNotInitialized',
  'TotalEventsOverflow',
  'TimestampOutOfRange',
  'InvalidSignature',
  'ContractPaused',
  'RateLimitExceeded',
  'SameOwner',
  'MaxLogsBelowCurrentCount',
  'CapAlreadyRemoved',
  'CapNeverSet',
  'NonceTooLow',
  'NoEventsForType',
  'InvalidPaginationParams',
  'InvalidWasmHash',
  'SubmitterBlocked',
  'CategoryTooLong',
  'ReentrancyDetected',
  'AlreadyInitialized',
  'NonceExhausted',
  'NonceWindowExceeded',
  'NonceResetNotExhausted',
  'SnapshotNotFound',
  'SnapshotVerificationFailed',
  'MetadataSchemaViolation',
  'InvalidVersion',
  'MaterialPassportAlreadyExists',
  'MaterialPassportNotFound',
  'InvalidLoopEventType',
  'InvalidFlowQuantity',
  'LcaProfileAlreadyExists',
  'LcaProfileNotFound',
  'InvalidLcaPhase',
  'InvalidImpactCategory',
  'LcaAlreadyFinalized',
  'LcaNotFinalized',
  'LcaNormRefNotFound',
  'LcaWeightingSchemeNotFound',
  'LcaDbEntryNotFound',
  'BioImpactNotFound',
  'BioOffsetNotFound',
  'InvalidLandUseType',
  'InvalidEcoServiceCat',
  'InvalidLandArea',
  'InvalidOffsetQuantity',
  'OffsetAlreadyRetired',
  'OffsetRetirementExceedsBalance',
  'SpeciesObservationNotFound',
  'WaterFootprintNotFound',
  'WaterRiskNotFound',
  'WaterStewardshipNotFound',
  'WaterDisclosureNotFound',
  'InvalidWaterSector',
  'InvalidWaterVolume',
  'InvalidScarcityFactor',
  'InvalidDisclosureYear',
  'EventOnLegalHold',
  'LegalHoldNotFound',
  'ComplianceExceptionActive',
  'EventAlreadyErased',
  'ErasureRequestNotFound',
  'ErasureRequestAlreadyDecided',
  'InvalidRetentionPeriod',
  'EmptyComplianceReason',
  'OperationalEventNotErasable',
  'UnauthorizedOpsRecorder',
  'RoleNotGranted',
  'InvalidDedupPolicy',
];

/**
 * contract: AuditLedger v0.1.0
 * idl: spec_version 1.0.0 (120 functions, 19 types, 75 errors, 43 events)
 */