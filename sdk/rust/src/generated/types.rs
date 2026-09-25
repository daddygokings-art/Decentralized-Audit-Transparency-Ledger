// GENERATED FILE — DO NOT EDIT.
// Regenerate with: npm run generate --workspace tools/sdk-codegen (or `sdk-codegen generate`).
// Source of truth: abi/audit-ledger.json
//
// Contract data types. Field names are preserved verbatim from the IDL so a
// value can be passed to the RPC layer without a translation step.

#![allow(clippy::derive_partial_eq_without_eq)]

use soroban_sdk::{Address, Bytes, BytesN, Duration, Map, String, Symbol, Timepoint, Vec};

#[doc = "Contract enum `SchemaFormat`."]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchemaFormat {
    /// Wire ordinal: 0
    JsonSchemaDraft7 = 0,
    /// Wire ordinal: 1
    JsonSchema201909 = 1,
    /// Wire ordinal: 2
    Protobuf = 2,
}

impl SchemaFormat {
    /// Decode a wire ordinal, or `None` when the ordinal is unknown.
    pub fn from_ordinal(value: u32) -> Option<Self> {
        match value {
            0 => Some(SchemaFormat::JsonSchemaDraft7),
            1 => Some(SchemaFormat::JsonSchema201909),
            2 => Some(SchemaFormat::Protobuf),
            _ => None,
        }
    }

    /// Wire ordinal for this variant.
    pub fn ordinal(self) -> u32 {
        self as u32
    }
}

#[doc = "Contract enum `SchemaCompatibility`."]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchemaCompatibility {
    /// Wire ordinal: 0
    Full = 0,
    /// Wire ordinal: 1
    Backward = 1,
    /// Wire ordinal: 2
    Forward = 2,
    /// Wire ordinal: 3
    Breaking = 3,
    /// Wire ordinal: 4
    Unknown = 4,
}

impl SchemaCompatibility {
    /// Decode a wire ordinal, or `None` when the ordinal is unknown.
    pub fn from_ordinal(value: u32) -> Option<Self> {
        match value {
            0 => Some(SchemaCompatibility::Full),
            1 => Some(SchemaCompatibility::Backward),
            2 => Some(SchemaCompatibility::Forward),
            3 => Some(SchemaCompatibility::Breaking),
            4 => Some(SchemaCompatibility::Unknown),
            _ => None,
        }
    }

    /// Wire ordinal for this variant.
    pub fn ordinal(self) -> u32 {
        self as u32
    }
}

#[doc = "Contract type `Schema`."]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Schema {
    pub format: super::SchemaFormat,
    pub version: u32,
    pub definition: Bytes,
    pub compatibility: super::SchemaCompatibility,
}

#[doc = "Contract type `MigrationFunction`."]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MigrationFunction {
    pub from_version: u32,
    pub to_version: u32,
    pub name: Symbol,
    pub body: Bytes,
}

#[doc = "Contract type `Event`."]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub index: u32,
    pub timestamp: u64,
    pub event_type: Symbol,
    pub category: Symbol,
    pub submitter: Address,
    pub metadata: Bytes,
    pub sub_event_type: Option<Symbol>,
    pub version: u32,
    pub event_hash: BytesN<32>,
    pub prev_hash: BytesN<32>,
    pub parent_event_id: Option<BytesN<32>>,
}

#[doc = "Contract type `EventHeader`."]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventHeader {
    pub index: u32,
    pub timestamp: u64,
    pub event_type: Symbol,
    pub submitter: Address,
}

#[doc = "Contract type `EventVersion`."]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventVersion {
    pub version: u32,
    pub data: super::Event,
    pub updated_at: u64,
    pub updated_by: Address,
}

#[doc = "Contract type `NonceState`."]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NonceState {
    pub last_nonce: u32,
    pub window_size: u32,
    pub max_nonce: u32,
}

#[doc = "Contract type `ContractStatistics`."]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractStatistics {
    pub total_events: u32,
    pub events_by_type: Vec<(Symbol, u32)>,
    pub events_last_hour: u32,
    pub events_last_day: u32,
    pub events_last_week: u32,
    pub top_submitters: Vec<(Address, u32)>,
}

#[doc = "Contract enum `ProposalAction`."]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProposalAction {
    /// Wire ordinal: 0
    TransferOwnership(Address),
    /// Wire ordinal: 1
    AddOwner(Address),
    /// Wire ordinal: 2
    RemoveOwner(Address),
    /// Wire ordinal: 3
    SetRequiredSignatures(u32),
    /// Wire ordinal: 4
    SetGlobalMaxLogs(u32),
    /// Wire ordinal: 5
    SetMetadataSchema(Symbol, Bytes),
    /// Wire ordinal: 6
    RollbackEvent(u32, u32),
    /// Wire ordinal: 7
    Pause,
    /// Wire ordinal: 8
    Unpause,
}

impl ProposalAction {
    /// Wire ordinal for this variant, by declaration order.
    pub fn ordinal(&self) -> u32 {
        match self {
            ProposalAction::TransferOwnership(_) => 0,
            ProposalAction::AddOwner(_) => 1,
            ProposalAction::RemoveOwner(_) => 2,
            ProposalAction::SetRequiredSignatures(_) => 3,
            ProposalAction::SetGlobalMaxLogs(_) => 4,
            ProposalAction::SetMetadataSchema(_, _) => 5,
            ProposalAction::RollbackEvent(_, _) => 6,
            ProposalAction::Pause => 7,
            ProposalAction::Unpause => 8,
        }
    }

    /// Every variant name, in wire order.
    pub const VARIANTS: &'static [&'static str] = &[
        "TransferOwnership",
        "AddOwner",
        "RemoveOwner",
        "SetRequiredSignatures",
        "SetGlobalMaxLogs",
        "SetMetadataSchema",
        "RollbackEvent",
        "Pause",
        "Unpause",
    ];
}

#[doc = "Contract type `Snapshot`."]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub id: u32,
    pub timestamp: u64,
    pub event_count: u32,
    pub event_hash: BytesN<32>,
    pub description: Bytes,
}

#[doc = "Contract type `TtlCleanupStats`."]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TtlCleanupStats {
    pub runs: u32,
    pub ttl_extensions: u32,
    pub cleaned: u32,
    pub last_run_ledger: u32,
}

#[doc = "Contract enum `Role`."]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Wire ordinal: 0
    Admin = 0,
    /// Wire ordinal: 1
    Auditor = 1,
    /// Wire ordinal: 2
    Submitter = 2,
    /// Wire ordinal: 3
    Viewer = 3,
}

impl Role {
    /// Decode a wire ordinal, or `None` when the ordinal is unknown.
    pub fn from_ordinal(value: u32) -> Option<Self> {
        match value {
            0 => Some(Role::Admin),
            1 => Some(Role::Auditor),
            2 => Some(Role::Submitter),
            3 => Some(Role::Viewer),
            _ => None,
        }
    }

    /// Wire ordinal for this variant.
    pub fn ordinal(self) -> u32 {
        self as u32
    }
}

#[doc = "Contract enum `DedupPolicy`."]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DedupPolicy {
    /// Wire ordinal: 0
    None = 0,
    /// Wire ordinal: 1
    ContentHash = 1,
    /// Wire ordinal: 2
    ContentHashWithTimestamp = 2,
    /// Wire ordinal: 3
    Custom = 3,
}

impl DedupPolicy {
    /// Decode a wire ordinal, or `None` when the ordinal is unknown.
    pub fn from_ordinal(value: u32) -> Option<Self> {
        match value {
            0 => Some(DedupPolicy::None),
            1 => Some(DedupPolicy::ContentHash),
            2 => Some(DedupPolicy::ContentHashWithTimestamp),
            3 => Some(DedupPolicy::Custom),
            _ => None,
        }
    }

    /// Wire ordinal for this variant.
    pub fn ordinal(self) -> u32 {
        self as u32
    }
}

#[doc = "Contract type `ArchiveConfig`."]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveConfig {
    pub offchain_storage: bool,
    pub base_url: Bytes,
    pub compression: u32,
}

#[doc = "Contract type `ArchivedEventRef`."]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchivedEventRef {
    pub id: BytesN<32>,
    pub index: u32,
    pub checksum: BytesN<32>,
    pub url: Bytes,
    pub archived_at: u64,
}

#[doc = "Contract type `ArchiveStats`."]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveStats {
    pub total_archived: u32,
    pub total_compressed: u32,
    pub total_offchain: u32,
}

#[doc = "Contract type `FieldChange`."]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldChange {
    pub field: Symbol,
    pub from: Bytes,
    pub to: Bytes,
}

#[doc = "Contract type `VersionComparison`."]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionComparison {
    pub index: u32,
    pub from_version: u32,
    pub to_version: u32,
    pub same: bool,
    pub changes: Vec<super::FieldChange>,
    pub from_hash: BytesN<32>,
    pub to_hash: BytesN<32>,
}

// contract: AuditLedger v0.1.0
// idl: spec_version 1.0.0 (120 functions, 19 types, 75 errors, 43 events)
