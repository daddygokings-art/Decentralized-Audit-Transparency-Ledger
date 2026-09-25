// GENERATED FILE — DO NOT EDIT.
// Regenerate with: npm run generate --workspace tools/sdk-codegen (or `sdk-codegen generate`).
// Source of truth: abi/audit-ledger.json
//
// Contract error codes. Codes are stable wire values; never renumber them.

/// Numeric contract error codes, ascending by wire value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ContractErrorCode {
    #[doc = "Contract error `CallerNotOwner`."]
    CallerNotOwner = 1,
    #[doc = "Contract error `GlobalMaxLogsReached`."]
    GlobalMaxLogsReached = 2,
    #[doc = "Contract error `EventTypeMaxLogsReached`."]
    EventTypeMaxLogsReached = 3,
    #[doc = "Contract error `EventDoesNotExist`."]
    EventDoesNotExist = 4,
    #[doc = "Contract error `EventTypeIndexOutOfBounds`."]
    EventTypeIndexOutOfBounds = 5,
    #[doc = "Contract error `NewOwnerIsZero`."]
    NewOwnerIsZero = 6,
    #[doc = "Contract error `CapNotSet`."]
    CapNotSet = 7,
    #[doc = "Contract error `MetadataTooLarge`."]
    MetadataTooLarge = 8,
    #[doc = "Contract error `ContractNotInitialized`."]
    ContractNotInitialized = 9,
    #[doc = "Contract error `TotalEventsOverflow`."]
    TotalEventsOverflow = 10,
    #[doc = "Contract error `TimestampOutOfRange`."]
    TimestampOutOfRange = 11,
    #[doc = "Contract error `InvalidSignature`."]
    InvalidSignature = 12,
    #[doc = "Contract error `ContractPaused`."]
    ContractPaused = 13,
    #[doc = "Contract error `RateLimitExceeded`."]
    RateLimitExceeded = 14,
    #[doc = "Contract error `SameOwner`."]
    SameOwner = 15,
    #[doc = "Contract error `MaxLogsBelowCurrentCount`."]
    MaxLogsBelowCurrentCount = 16,
    #[doc = "Contract error `CapAlreadyRemoved`."]
    CapAlreadyRemoved = 17,
    #[doc = "Contract error `CapNeverSet`."]
    CapNeverSet = 18,
    #[doc = "Contract error `NonceTooLow`."]
    NonceTooLow = 19,
    #[doc = "Contract error `NoEventsForType`."]
    NoEventsForType = 20,
    #[doc = "Contract error `InvalidPaginationParams`."]
    InvalidPaginationParams = 21,
    #[doc = "Contract error `InvalidWasmHash`."]
    InvalidWasmHash = 22,
    #[doc = "Contract error `SubmitterBlocked`."]
    SubmitterBlocked = 23,
    #[doc = "Contract error `CategoryTooLong`."]
    CategoryTooLong = 24,
    #[doc = "Contract error `ReentrancyDetected`."]
    ReentrancyDetected = 25,
    #[doc = "Contract error `AlreadyInitialized`."]
    AlreadyInitialized = 26,
    #[doc = "Contract error `NonceExhausted`."]
    NonceExhausted = 27,
    #[doc = "Contract error `NonceWindowExceeded`."]
    NonceWindowExceeded = 28,
    #[doc = "Contract error `NonceResetNotExhausted`."]
    NonceResetNotExhausted = 29,
    #[doc = "Contract error `SnapshotNotFound`."]
    SnapshotNotFound = 30,
    #[doc = "Contract error `SnapshotVerificationFailed`."]
    SnapshotVerificationFailed = 31,
    #[doc = "Contract error `MetadataSchemaViolation`."]
    MetadataSchemaViolation = 32,
    #[doc = "Contract error `InvalidVersion`."]
    InvalidVersion = 33,
    #[doc = "Contract error `MaterialPassportAlreadyExists`."]
    MaterialPassportAlreadyExists = 34,
    #[doc = "Contract error `MaterialPassportNotFound`."]
    MaterialPassportNotFound = 35,
    #[doc = "Contract error `InvalidLoopEventType`."]
    InvalidLoopEventType = 36,
    #[doc = "Contract error `InvalidFlowQuantity`."]
    InvalidFlowQuantity = 37,
    #[doc = "Contract error `LcaProfileAlreadyExists`."]
    LcaProfileAlreadyExists = 38,
    #[doc = "Contract error `LcaProfileNotFound`."]
    LcaProfileNotFound = 39,
    #[doc = "Contract error `InvalidLcaPhase`."]
    InvalidLcaPhase = 40,
    #[doc = "Contract error `InvalidImpactCategory`."]
    InvalidImpactCategory = 41,
    #[doc = "Contract error `LcaAlreadyFinalized`."]
    LcaAlreadyFinalized = 42,
    #[doc = "Contract error `LcaNotFinalized`."]
    LcaNotFinalized = 43,
    #[doc = "Contract error `LcaNormRefNotFound`."]
    LcaNormRefNotFound = 44,
    #[doc = "Contract error `LcaWeightingSchemeNotFound`."]
    LcaWeightingSchemeNotFound = 45,
    #[doc = "Contract error `LcaDbEntryNotFound`."]
    LcaDbEntryNotFound = 46,
    #[doc = "Contract error `BioImpactNotFound`."]
    BioImpactNotFound = 47,
    #[doc = "Contract error `BioOffsetNotFound`."]
    BioOffsetNotFound = 48,
    #[doc = "Contract error `InvalidLandUseType`."]
    InvalidLandUseType = 49,
    #[doc = "Contract error `InvalidEcoServiceCat`."]
    InvalidEcoServiceCat = 50,
    #[doc = "Contract error `InvalidLandArea`."]
    InvalidLandArea = 51,
    #[doc = "Contract error `InvalidOffsetQuantity`."]
    InvalidOffsetQuantity = 52,
    #[doc = "Contract error `OffsetAlreadyRetired`."]
    OffsetAlreadyRetired = 53,
    #[doc = "Contract error `OffsetRetirementExceedsBalance`."]
    OffsetRetirementExceedsBalance = 54,
    #[doc = "Contract error `SpeciesObservationNotFound`."]
    SpeciesObservationNotFound = 55,
    #[doc = "Contract error `WaterFootprintNotFound`."]
    WaterFootprintNotFound = 56,
    #[doc = "Contract error `WaterRiskNotFound`."]
    WaterRiskNotFound = 57,
    #[doc = "Contract error `WaterStewardshipNotFound`."]
    WaterStewardshipNotFound = 58,
    #[doc = "Contract error `WaterDisclosureNotFound`."]
    WaterDisclosureNotFound = 59,
    #[doc = "Contract error `InvalidWaterSector`."]
    InvalidWaterSector = 60,
    #[doc = "Contract error `InvalidWaterVolume`."]
    InvalidWaterVolume = 61,
    #[doc = "Contract error `InvalidScarcityFactor`."]
    InvalidScarcityFactor = 62,
    #[doc = "Contract error `InvalidDisclosureYear`."]
    InvalidDisclosureYear = 63,
    #[doc = "Contract error `EventOnLegalHold`."]
    EventOnLegalHold = 64,
    #[doc = "Contract error `LegalHoldNotFound`."]
    LegalHoldNotFound = 65,
    #[doc = "Contract error `ComplianceExceptionActive`."]
    ComplianceExceptionActive = 66,
    #[doc = "Contract error `EventAlreadyErased`."]
    EventAlreadyErased = 67,
    #[doc = "Contract error `ErasureRequestNotFound`."]
    ErasureRequestNotFound = 68,
    #[doc = "Contract error `ErasureRequestAlreadyDecided`."]
    ErasureRequestAlreadyDecided = 69,
    #[doc = "Contract error `InvalidRetentionPeriod`."]
    InvalidRetentionPeriod = 70,
    #[doc = "Contract error `EmptyComplianceReason`."]
    EmptyComplianceReason = 71,
    #[doc = "Contract error `OperationalEventNotErasable`."]
    OperationalEventNotErasable = 72,
    #[doc = "Contract error `UnauthorizedOpsRecorder`."]
    UnauthorizedOpsRecorder = 73,
    #[doc = "Contract error `RoleNotGranted`."]
    RoleNotGranted = 74,
    #[doc = "Contract error `InvalidDedupPolicy`."]
    InvalidDedupPolicy = 75,
}

impl ContractErrorCode {
    /// Every error code known to this SDK, in wire order.
    pub const ALL: [ContractErrorCode; 75] = [
        ContractErrorCode::CallerNotOwner,
        ContractErrorCode::GlobalMaxLogsReached,
        ContractErrorCode::EventTypeMaxLogsReached,
        ContractErrorCode::EventDoesNotExist,
        ContractErrorCode::EventTypeIndexOutOfBounds,
        ContractErrorCode::NewOwnerIsZero,
        ContractErrorCode::CapNotSet,
        ContractErrorCode::MetadataTooLarge,
        ContractErrorCode::ContractNotInitialized,
        ContractErrorCode::TotalEventsOverflow,
        ContractErrorCode::TimestampOutOfRange,
        ContractErrorCode::InvalidSignature,
        ContractErrorCode::ContractPaused,
        ContractErrorCode::RateLimitExceeded,
        ContractErrorCode::SameOwner,
        ContractErrorCode::MaxLogsBelowCurrentCount,
        ContractErrorCode::CapAlreadyRemoved,
        ContractErrorCode::CapNeverSet,
        ContractErrorCode::NonceTooLow,
        ContractErrorCode::NoEventsForType,
        ContractErrorCode::InvalidPaginationParams,
        ContractErrorCode::InvalidWasmHash,
        ContractErrorCode::SubmitterBlocked,
        ContractErrorCode::CategoryTooLong,
        ContractErrorCode::ReentrancyDetected,
        ContractErrorCode::AlreadyInitialized,
        ContractErrorCode::NonceExhausted,
        ContractErrorCode::NonceWindowExceeded,
        ContractErrorCode::NonceResetNotExhausted,
        ContractErrorCode::SnapshotNotFound,
        ContractErrorCode::SnapshotVerificationFailed,
        ContractErrorCode::MetadataSchemaViolation,
        ContractErrorCode::InvalidVersion,
        ContractErrorCode::MaterialPassportAlreadyExists,
        ContractErrorCode::MaterialPassportNotFound,
        ContractErrorCode::InvalidLoopEventType,
        ContractErrorCode::InvalidFlowQuantity,
        ContractErrorCode::LcaProfileAlreadyExists,
        ContractErrorCode::LcaProfileNotFound,
        ContractErrorCode::InvalidLcaPhase,
        ContractErrorCode::InvalidImpactCategory,
        ContractErrorCode::LcaAlreadyFinalized,
        ContractErrorCode::LcaNotFinalized,
        ContractErrorCode::LcaNormRefNotFound,
        ContractErrorCode::LcaWeightingSchemeNotFound,
        ContractErrorCode::LcaDbEntryNotFound,
        ContractErrorCode::BioImpactNotFound,
        ContractErrorCode::BioOffsetNotFound,
        ContractErrorCode::InvalidLandUseType,
        ContractErrorCode::InvalidEcoServiceCat,
        ContractErrorCode::InvalidLandArea,
        ContractErrorCode::InvalidOffsetQuantity,
        ContractErrorCode::OffsetAlreadyRetired,
        ContractErrorCode::OffsetRetirementExceedsBalance,
        ContractErrorCode::SpeciesObservationNotFound,
        ContractErrorCode::WaterFootprintNotFound,
        ContractErrorCode::WaterRiskNotFound,
        ContractErrorCode::WaterStewardshipNotFound,
        ContractErrorCode::WaterDisclosureNotFound,
        ContractErrorCode::InvalidWaterSector,
        ContractErrorCode::InvalidWaterVolume,
        ContractErrorCode::InvalidScarcityFactor,
        ContractErrorCode::InvalidDisclosureYear,
        ContractErrorCode::EventOnLegalHold,
        ContractErrorCode::LegalHoldNotFound,
        ContractErrorCode::ComplianceExceptionActive,
        ContractErrorCode::EventAlreadyErased,
        ContractErrorCode::ErasureRequestNotFound,
        ContractErrorCode::ErasureRequestAlreadyDecided,
        ContractErrorCode::InvalidRetentionPeriod,
        ContractErrorCode::EmptyComplianceReason,
        ContractErrorCode::OperationalEventNotErasable,
        ContractErrorCode::UnauthorizedOpsRecorder,
        ContractErrorCode::RoleNotGranted,
        ContractErrorCode::InvalidDedupPolicy,
    ];

    /// Decode a wire code, or `None` when the code is unknown to this SDK.
    pub fn from_code(code: u32) -> Option<Self> {
        match code {
            1 => Some(ContractErrorCode::CallerNotOwner),
            2 => Some(ContractErrorCode::GlobalMaxLogsReached),
            3 => Some(ContractErrorCode::EventTypeMaxLogsReached),
            4 => Some(ContractErrorCode::EventDoesNotExist),
            5 => Some(ContractErrorCode::EventTypeIndexOutOfBounds),
            6 => Some(ContractErrorCode::NewOwnerIsZero),
            7 => Some(ContractErrorCode::CapNotSet),
            8 => Some(ContractErrorCode::MetadataTooLarge),
            9 => Some(ContractErrorCode::ContractNotInitialized),
            10 => Some(ContractErrorCode::TotalEventsOverflow),
            11 => Some(ContractErrorCode::TimestampOutOfRange),
            12 => Some(ContractErrorCode::InvalidSignature),
            13 => Some(ContractErrorCode::ContractPaused),
            14 => Some(ContractErrorCode::RateLimitExceeded),
            15 => Some(ContractErrorCode::SameOwner),
            16 => Some(ContractErrorCode::MaxLogsBelowCurrentCount),
            17 => Some(ContractErrorCode::CapAlreadyRemoved),
            18 => Some(ContractErrorCode::CapNeverSet),
            19 => Some(ContractErrorCode::NonceTooLow),
            20 => Some(ContractErrorCode::NoEventsForType),
            21 => Some(ContractErrorCode::InvalidPaginationParams),
            22 => Some(ContractErrorCode::InvalidWasmHash),
            23 => Some(ContractErrorCode::SubmitterBlocked),
            24 => Some(ContractErrorCode::CategoryTooLong),
            25 => Some(ContractErrorCode::ReentrancyDetected),
            26 => Some(ContractErrorCode::AlreadyInitialized),
            27 => Some(ContractErrorCode::NonceExhausted),
            28 => Some(ContractErrorCode::NonceWindowExceeded),
            29 => Some(ContractErrorCode::NonceResetNotExhausted),
            30 => Some(ContractErrorCode::SnapshotNotFound),
            31 => Some(ContractErrorCode::SnapshotVerificationFailed),
            32 => Some(ContractErrorCode::MetadataSchemaViolation),
            33 => Some(ContractErrorCode::InvalidVersion),
            34 => Some(ContractErrorCode::MaterialPassportAlreadyExists),
            35 => Some(ContractErrorCode::MaterialPassportNotFound),
            36 => Some(ContractErrorCode::InvalidLoopEventType),
            37 => Some(ContractErrorCode::InvalidFlowQuantity),
            38 => Some(ContractErrorCode::LcaProfileAlreadyExists),
            39 => Some(ContractErrorCode::LcaProfileNotFound),
            40 => Some(ContractErrorCode::InvalidLcaPhase),
            41 => Some(ContractErrorCode::InvalidImpactCategory),
            42 => Some(ContractErrorCode::LcaAlreadyFinalized),
            43 => Some(ContractErrorCode::LcaNotFinalized),
            44 => Some(ContractErrorCode::LcaNormRefNotFound),
            45 => Some(ContractErrorCode::LcaWeightingSchemeNotFound),
            46 => Some(ContractErrorCode::LcaDbEntryNotFound),
            47 => Some(ContractErrorCode::BioImpactNotFound),
            48 => Some(ContractErrorCode::BioOffsetNotFound),
            49 => Some(ContractErrorCode::InvalidLandUseType),
            50 => Some(ContractErrorCode::InvalidEcoServiceCat),
            51 => Some(ContractErrorCode::InvalidLandArea),
            52 => Some(ContractErrorCode::InvalidOffsetQuantity),
            53 => Some(ContractErrorCode::OffsetAlreadyRetired),
            54 => Some(ContractErrorCode::OffsetRetirementExceedsBalance),
            55 => Some(ContractErrorCode::SpeciesObservationNotFound),
            56 => Some(ContractErrorCode::WaterFootprintNotFound),
            57 => Some(ContractErrorCode::WaterRiskNotFound),
            58 => Some(ContractErrorCode::WaterStewardshipNotFound),
            59 => Some(ContractErrorCode::WaterDisclosureNotFound),
            60 => Some(ContractErrorCode::InvalidWaterSector),
            61 => Some(ContractErrorCode::InvalidWaterVolume),
            62 => Some(ContractErrorCode::InvalidScarcityFactor),
            63 => Some(ContractErrorCode::InvalidDisclosureYear),
            64 => Some(ContractErrorCode::EventOnLegalHold),
            65 => Some(ContractErrorCode::LegalHoldNotFound),
            66 => Some(ContractErrorCode::ComplianceExceptionActive),
            67 => Some(ContractErrorCode::EventAlreadyErased),
            68 => Some(ContractErrorCode::ErasureRequestNotFound),
            69 => Some(ContractErrorCode::ErasureRequestAlreadyDecided),
            70 => Some(ContractErrorCode::InvalidRetentionPeriod),
            71 => Some(ContractErrorCode::EmptyComplianceReason),
            72 => Some(ContractErrorCode::OperationalEventNotErasable),
            73 => Some(ContractErrorCode::UnauthorizedOpsRecorder),
            74 => Some(ContractErrorCode::RoleNotGranted),
            75 => Some(ContractErrorCode::InvalidDedupPolicy),
            _ => None,
        }
    }

    /// Wire code for this error.
    pub fn code(self) -> u32 {
        self as u32
    }

    /// Canonical name, identical to the Rust variant name.
    pub fn name(self) -> &'static str {
        match self {
            ContractErrorCode::CallerNotOwner => "CallerNotOwner",
            ContractErrorCode::GlobalMaxLogsReached => "GlobalMaxLogsReached",
            ContractErrorCode::EventTypeMaxLogsReached => "EventTypeMaxLogsReached",
            ContractErrorCode::EventDoesNotExist => "EventDoesNotExist",
            ContractErrorCode::EventTypeIndexOutOfBounds => "EventTypeIndexOutOfBounds",
            ContractErrorCode::NewOwnerIsZero => "NewOwnerIsZero",
            ContractErrorCode::CapNotSet => "CapNotSet",
            ContractErrorCode::MetadataTooLarge => "MetadataTooLarge",
            ContractErrorCode::ContractNotInitialized => "ContractNotInitialized",
            ContractErrorCode::TotalEventsOverflow => "TotalEventsOverflow",
            ContractErrorCode::TimestampOutOfRange => "TimestampOutOfRange",
            ContractErrorCode::InvalidSignature => "InvalidSignature",
            ContractErrorCode::ContractPaused => "ContractPaused",
            ContractErrorCode::RateLimitExceeded => "RateLimitExceeded",
            ContractErrorCode::SameOwner => "SameOwner",
            ContractErrorCode::MaxLogsBelowCurrentCount => "MaxLogsBelowCurrentCount",
            ContractErrorCode::CapAlreadyRemoved => "CapAlreadyRemoved",
            ContractErrorCode::CapNeverSet => "CapNeverSet",
            ContractErrorCode::NonceTooLow => "NonceTooLow",
            ContractErrorCode::NoEventsForType => "NoEventsForType",
            ContractErrorCode::InvalidPaginationParams => "InvalidPaginationParams",
            ContractErrorCode::InvalidWasmHash => "InvalidWasmHash",
            ContractErrorCode::SubmitterBlocked => "SubmitterBlocked",
            ContractErrorCode::CategoryTooLong => "CategoryTooLong",
            ContractErrorCode::ReentrancyDetected => "ReentrancyDetected",
            ContractErrorCode::AlreadyInitialized => "AlreadyInitialized",
            ContractErrorCode::NonceExhausted => "NonceExhausted",
            ContractErrorCode::NonceWindowExceeded => "NonceWindowExceeded",
            ContractErrorCode::NonceResetNotExhausted => "NonceResetNotExhausted",
            ContractErrorCode::SnapshotNotFound => "SnapshotNotFound",
            ContractErrorCode::SnapshotVerificationFailed => "SnapshotVerificationFailed",
            ContractErrorCode::MetadataSchemaViolation => "MetadataSchemaViolation",
            ContractErrorCode::InvalidVersion => "InvalidVersion",
            ContractErrorCode::MaterialPassportAlreadyExists => "MaterialPassportAlreadyExists",
            ContractErrorCode::MaterialPassportNotFound => "MaterialPassportNotFound",
            ContractErrorCode::InvalidLoopEventType => "InvalidLoopEventType",
            ContractErrorCode::InvalidFlowQuantity => "InvalidFlowQuantity",
            ContractErrorCode::LcaProfileAlreadyExists => "LcaProfileAlreadyExists",
            ContractErrorCode::LcaProfileNotFound => "LcaProfileNotFound",
            ContractErrorCode::InvalidLcaPhase => "InvalidLcaPhase",
            ContractErrorCode::InvalidImpactCategory => "InvalidImpactCategory",
            ContractErrorCode::LcaAlreadyFinalized => "LcaAlreadyFinalized",
            ContractErrorCode::LcaNotFinalized => "LcaNotFinalized",
            ContractErrorCode::LcaNormRefNotFound => "LcaNormRefNotFound",
            ContractErrorCode::LcaWeightingSchemeNotFound => "LcaWeightingSchemeNotFound",
            ContractErrorCode::LcaDbEntryNotFound => "LcaDbEntryNotFound",
            ContractErrorCode::BioImpactNotFound => "BioImpactNotFound",
            ContractErrorCode::BioOffsetNotFound => "BioOffsetNotFound",
            ContractErrorCode::InvalidLandUseType => "InvalidLandUseType",
            ContractErrorCode::InvalidEcoServiceCat => "InvalidEcoServiceCat",
            ContractErrorCode::InvalidLandArea => "InvalidLandArea",
            ContractErrorCode::InvalidOffsetQuantity => "InvalidOffsetQuantity",
            ContractErrorCode::OffsetAlreadyRetired => "OffsetAlreadyRetired",
            ContractErrorCode::OffsetRetirementExceedsBalance => "OffsetRetirementExceedsBalance",
            ContractErrorCode::SpeciesObservationNotFound => "SpeciesObservationNotFound",
            ContractErrorCode::WaterFootprintNotFound => "WaterFootprintNotFound",
            ContractErrorCode::WaterRiskNotFound => "WaterRiskNotFound",
            ContractErrorCode::WaterStewardshipNotFound => "WaterStewardshipNotFound",
            ContractErrorCode::WaterDisclosureNotFound => "WaterDisclosureNotFound",
            ContractErrorCode::InvalidWaterSector => "InvalidWaterSector",
            ContractErrorCode::InvalidWaterVolume => "InvalidWaterVolume",
            ContractErrorCode::InvalidScarcityFactor => "InvalidScarcityFactor",
            ContractErrorCode::InvalidDisclosureYear => "InvalidDisclosureYear",
            ContractErrorCode::EventOnLegalHold => "EventOnLegalHold",
            ContractErrorCode::LegalHoldNotFound => "LegalHoldNotFound",
            ContractErrorCode::ComplianceExceptionActive => "ComplianceExceptionActive",
            ContractErrorCode::EventAlreadyErased => "EventAlreadyErased",
            ContractErrorCode::ErasureRequestNotFound => "ErasureRequestNotFound",
            ContractErrorCode::ErasureRequestAlreadyDecided => "ErasureRequestAlreadyDecided",
            ContractErrorCode::InvalidRetentionPeriod => "InvalidRetentionPeriod",
            ContractErrorCode::EmptyComplianceReason => "EmptyComplianceReason",
            ContractErrorCode::OperationalEventNotErasable => "OperationalEventNotErasable",
            ContractErrorCode::UnauthorizedOpsRecorder => "UnauthorizedOpsRecorder",
            ContractErrorCode::RoleNotGranted => "RoleNotGranted",
            ContractErrorCode::InvalidDedupPolicy => "InvalidDedupPolicy",
        }
    }

    /// Human-readable description including the wire code.
    pub fn describe(self) -> String {
        alloc::format!("{} ({})", self.name(), self.code())
    }
}

// contract: AuditLedger v0.1.0
// idl: spec_version 1.0.0 (120 functions, 19 types, 75 errors, 43 events)
