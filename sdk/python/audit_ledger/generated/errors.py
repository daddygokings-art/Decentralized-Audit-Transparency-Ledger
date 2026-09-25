# GENERATED FILE — DO NOT EDIT.
# Regenerate with: npm run generate --workspace tools/sdk-codegen (or `sdk-codegen generate`).
# Source of truth: abi/audit-ledger.json
"""

Contract error codes. Codes are stable wire values; never renumber them.
"""

from enum import IntEnum
from typing import Dict, Optional

__all__ = ["ContractErrorCode", "contract_error_name", "describe_contract_error", "CONTRACT_ERROR_NAMES"]


class ContractErrorCode(IntEnum):
    """Numeric contract error codes, ascending by wire value."""

    CallerNotOwner = 1
    GlobalMaxLogsReached = 2
    EventTypeMaxLogsReached = 3
    EventDoesNotExist = 4
    EventTypeIndexOutOfBounds = 5
    NewOwnerIsZero = 6
    CapNotSet = 7
    MetadataTooLarge = 8
    ContractNotInitialized = 9
    TotalEventsOverflow = 10
    TimestampOutOfRange = 11
    InvalidSignature = 12
    ContractPaused = 13
    RateLimitExceeded = 14
    SameOwner = 15
    MaxLogsBelowCurrentCount = 16
    CapAlreadyRemoved = 17
    CapNeverSet = 18
    NonceTooLow = 19
    NoEventsForType = 20
    InvalidPaginationParams = 21
    InvalidWasmHash = 22
    SubmitterBlocked = 23
    CategoryTooLong = 24
    ReentrancyDetected = 25
    AlreadyInitialized = 26
    NonceExhausted = 27
    NonceWindowExceeded = 28
    NonceResetNotExhausted = 29
    SnapshotNotFound = 30
    SnapshotVerificationFailed = 31
    MetadataSchemaViolation = 32
    InvalidVersion = 33
    MaterialPassportAlreadyExists = 34
    MaterialPassportNotFound = 35
    InvalidLoopEventType = 36
    InvalidFlowQuantity = 37
    LcaProfileAlreadyExists = 38
    LcaProfileNotFound = 39
    InvalidLcaPhase = 40
    InvalidImpactCategory = 41
    LcaAlreadyFinalized = 42
    LcaNotFinalized = 43
    LcaNormRefNotFound = 44
    LcaWeightingSchemeNotFound = 45
    LcaDbEntryNotFound = 46
    BioImpactNotFound = 47
    BioOffsetNotFound = 48
    InvalidLandUseType = 49
    InvalidEcoServiceCat = 50
    InvalidLandArea = 51
    InvalidOffsetQuantity = 52
    OffsetAlreadyRetired = 53
    OffsetRetirementExceedsBalance = 54
    SpeciesObservationNotFound = 55
    WaterFootprintNotFound = 56
    WaterRiskNotFound = 57
    WaterStewardshipNotFound = 58
    WaterDisclosureNotFound = 59
    InvalidWaterSector = 60
    InvalidWaterVolume = 61
    InvalidScarcityFactor = 62
    InvalidDisclosureYear = 63
    EventOnLegalHold = 64
    LegalHoldNotFound = 65
    ComplianceExceptionActive = 66
    EventAlreadyErased = 67
    ErasureRequestNotFound = 68
    ErasureRequestAlreadyDecided = 69
    InvalidRetentionPeriod = 70
    EmptyComplianceReason = 71
    OperationalEventNotErasable = 72
    UnauthorizedOpsRecorder = 73
    RoleNotGranted = 74
    InvalidDedupPolicy = 75


CONTRACT_ERROR_NAMES: Dict[int, str] = {
    1: "CallerNotOwner",
    2: "GlobalMaxLogsReached",
    3: "EventTypeMaxLogsReached",
    4: "EventDoesNotExist",
    5: "EventTypeIndexOutOfBounds",
    6: "NewOwnerIsZero",
    7: "CapNotSet",
    8: "MetadataTooLarge",
    9: "ContractNotInitialized",
    10: "TotalEventsOverflow",
    11: "TimestampOutOfRange",
    12: "InvalidSignature",
    13: "ContractPaused",
    14: "RateLimitExceeded",
    15: "SameOwner",
    16: "MaxLogsBelowCurrentCount",
    17: "CapAlreadyRemoved",
    18: "CapNeverSet",
    19: "NonceTooLow",
    20: "NoEventsForType",
    21: "InvalidPaginationParams",
    22: "InvalidWasmHash",
    23: "SubmitterBlocked",
    24: "CategoryTooLong",
    25: "ReentrancyDetected",
    26: "AlreadyInitialized",
    27: "NonceExhausted",
    28: "NonceWindowExceeded",
    29: "NonceResetNotExhausted",
    30: "SnapshotNotFound",
    31: "SnapshotVerificationFailed",
    32: "MetadataSchemaViolation",
    33: "InvalidVersion",
    34: "MaterialPassportAlreadyExists",
    35: "MaterialPassportNotFound",
    36: "InvalidLoopEventType",
    37: "InvalidFlowQuantity",
    38: "LcaProfileAlreadyExists",
    39: "LcaProfileNotFound",
    40: "InvalidLcaPhase",
    41: "InvalidImpactCategory",
    42: "LcaAlreadyFinalized",
    43: "LcaNotFinalized",
    44: "LcaNormRefNotFound",
    45: "LcaWeightingSchemeNotFound",
    46: "LcaDbEntryNotFound",
    47: "BioImpactNotFound",
    48: "BioOffsetNotFound",
    49: "InvalidLandUseType",
    50: "InvalidEcoServiceCat",
    51: "InvalidLandArea",
    52: "InvalidOffsetQuantity",
    53: "OffsetAlreadyRetired",
    54: "OffsetRetirementExceedsBalance",
    55: "SpeciesObservationNotFound",
    56: "WaterFootprintNotFound",
    57: "WaterRiskNotFound",
    58: "WaterStewardshipNotFound",
    59: "WaterDisclosureNotFound",
    60: "InvalidWaterSector",
    61: "InvalidWaterVolume",
    62: "InvalidScarcityFactor",
    63: "InvalidDisclosureYear",
    64: "EventOnLegalHold",
    65: "LegalHoldNotFound",
    66: "ComplianceExceptionActive",
    67: "EventAlreadyErased",
    68: "ErasureRequestNotFound",
    69: "ErasureRequestAlreadyDecided",
    70: "InvalidRetentionPeriod",
    71: "EmptyComplianceReason",
    72: "OperationalEventNotErasable",
    73: "UnauthorizedOpsRecorder",
    74: "RoleNotGranted",
    75: "InvalidDedupPolicy",
}


def contract_error_name(code: int) -> Optional[str]:
    """Return the canonical error name for a wire code, or `None`."""
    return CONTRACT_ERROR_NAMES.get(int(code))


def describe_contract_error(code: int) -> str:
    """Return a human-readable description of a wire error code."""
    name = contract_error_name(code)
    if name is None:
        return f"Unknown contract error code {code}"
    return f"{name} ({code})"

# contract: AuditLedger v0.1.0
# idl: spec_version 1.0.0 (120 functions, 19 types, 75 errors, 43 events)