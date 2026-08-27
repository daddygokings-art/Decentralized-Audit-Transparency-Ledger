//! Regulator-specific audit trail extensions with selective disclosure,
//! data sharing agreements, and compliance standards (ISA 3000, SOC2).
//!
//! This module provides:
//! - Regulator-specific event types and compliance classifications
//! - Tamper-evidence verification via hash chains
//! - Selective disclosure with cryptographic proofs
//! - Data sharing agreement management
//! - Compliance standard validators (ISA 3000, SOC2)

use soroban_sdk::{contracttype, BytesN, Address, Symbol, Bytes, Vec, Env};

/// Compliance audit standards supported by the system
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ComplianceStandard {
    /// ISA 3000: International Standard on Assurance Engagements
    ISA3000 = 0,
    /// SOC 2: Service Organization Control Framework
    SOC2 = 1,
    /// GDPR compliance framework
    GDPR = 2,
    /// SOX: Sarbanes-Oxley Act
    SOX = 3,
}

/// Regulator role and permission levels
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RegulatorRole {
    /// View-only access to audit events
    Auditor = 0,
    /// Can request disclosures and manage access
    RegulatorOfficer = 1,
    /// Full administrative access
    RegulatoryAdmin = 2,
}

/// Data sensitivity level for selective disclosure
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum SensitivityLevel {
    /// Public information
    Public = 0,
    /// Internal use only
    Internal = 1,
    /// Confidential
    Confidential = 2,
    /// Highly restricted
    Restricted = 3,
}

/// Event classification for regulatory purposes
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegulatoryEventClass {
    /// Standard this classification belongs to (ISA 3000, SOC2, etc.)
    pub standard: ComplianceStandard,
    /// Classification code (e.g., "CC6.1", "A1.1" for ISA 3000)
    pub control_code: Symbol,
    /// Whether this event demonstrates control effectiveness
    pub demonstrates_control: bool,
    /// Required retention period in ledgers
    pub retention_ledgers: u32,
    /// Sensitivity level for selective disclosure
    pub sensitivity: SensitivityLevel,
}

/// Tamper-evidence proof for a single event
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TamperProof {
    /// Event index in the global log
    pub event_index: u32,
    /// SHA-256 hash of this event
    pub event_hash: BytesN<32>,
    /// SHA-256 hash of the previous event
    pub prev_hash: BytesN<32>,
    /// Timestamp of this event
    pub timestamp: u64,
    /// Timestamp of the next event (0 if not found)
    pub next_timestamp: u64,
    /// Whether the hash chain is valid (event_hash correctly computed)
    pub chain_valid: bool,
}

/// Selective disclosure proof for an event
/// Proves certain facts about an event without revealing full metadata
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectiveDisclosureProof {
    /// The event being disclosed
    pub event_index: u32,
    /// Merkle root of disclosed fields
    pub disclosed_root: BytesN<32>,
    /// Merkle root of all event fields
    pub complete_root: BytesN<32>,
    /// List of disclosed field names (e.g., ["timestamp", "submitter"])
    pub disclosed_fields: Vec<Symbol>,
    /// Cryptographic proof of inclusion for disclosed fields
    pub merkle_proof: Vec<BytesN<32>>,
}

/// Data Sharing Agreement between entity and regulator
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataSharingAgreement {
    /// Unique identifier for this agreement
    pub id: BytesN<32>,
    /// Entity providing the audit data
    pub data_provider: Address,
    /// Regulator receiving access
    pub regulator_address: Address,
    /// When this agreement takes effect (ledger sequence number)
    pub effective_ledger: u32,
    /// When this agreement expires (0 = no expiry)
    pub expiry_ledger: u32,
    /// Allowed compliance standards under this agreement
    pub standards: Vec<ComplianceStandard>,
    /// Permissible event types to access
    pub allowed_event_types: Vec<Symbol>,
    /// Regulator role granted
    pub role: RegulatorRole,
    /// Minimum sensitivity level that can be disclosed
    pub min_sensitivity: SensitivityLevel,
    /// Whether agreement is currently active
    pub active: bool,
    /// Signature of the agreement by both parties
    pub signature_provider: BytesN<64>,
    pub signature_regulator: BytesN<64>,
}

/// Regulator portal access request
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessRequest {
    /// Unique request ID
    pub id: BytesN<32>,
    /// Regulator requesting access
    pub requester: Address,
    /// Entity whose data is requested
    pub data_owner: Address,
    /// Compliance standard for which access is needed
    pub standard: ComplianceStandard,
    /// Requested event types to access
    pub event_types: Vec<Symbol>,
    /// Legal basis for the request (e.g., regulatory authority, audit mandate)
    pub legal_basis: Bytes,
    /// Proposed data sharing agreement terms
    pub proposed_terms: DataSharingAgreement,
    /// Request status: 0=pending, 1=approved, 2=rejected
    pub status: u32,
    /// Timestamp when request was created
    pub created_at: u64,
    /// Timestamp when request was resolved (0 if pending)
    pub resolved_at: u64,
}

/// Compliance audit report generated for a regulator
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComplianceReport {
    /// Report identifier
    pub id: BytesN<32>,
    /// Report type (ISA 3000, SOC2, etc.)
    pub standard: ComplianceStandard,
    /// Entity being audited
    pub audit_subject: Address,
    /// Regulator issuing report
    pub issuer: Address,
    /// Report generation date (ledger timestamp)
    pub generated_at: u64,
    /// Report status: 0=draft, 1=pending_review, 2=published, 3=withdrawn
    pub status: u32,
    /// Total events examined
    pub events_examined: u32,
    /// Control objectives tested
    pub objectives_tested: Vec<Symbol>,
    /// Controls found to be operating effectively
    pub controls_operating: u32,
    /// Controls found deficient
    pub controls_deficient: u32,
    /// Key findings (as opaque bytes for privacy)
    pub findings_summary_hash: BytesN<32>,
}

/// Audit trail query filter for regulators
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditFilter {
    /// Event types to include (empty = all)
    pub event_types: Vec<Symbol>,
    /// Start timestamp (inclusive)
    pub start_time: u64,
    /// End timestamp (inclusive)
    pub end_time: u64,
    /// Submitter address filter (empty = all)
    pub submitter: Option<Address>,
    /// Minimum sensitivity to include
    pub min_sensitivity: SensitivityLevel,
    /// Whether to include only control-demonstrating events
    pub only_control_events: bool,
}

/// Audit log entry with regulator metadata
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegulatorAuditEntry {
    /// Event index in global log
    pub event_index: u32,
    /// Event hash for verification
    pub event_hash: BytesN<32>,
    /// Event timestamp
    pub timestamp: u64,
    /// Event type
    pub event_type: Symbol,
    /// Submitter address
    pub submitter: Address,
    /// Regulatory classification
    pub regulatory_class: Option<RegulatoryEventClass>,
    /// Sensitivity level
    pub sensitivity: SensitivityLevel,
    /// Whether this event demonstrates control effectiveness
    pub control_event: bool,
}

/// Enum for regulator contract errors
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RegulatorError {
    /// Regulator not authorized for this operation
    UnauthorizedRegulator = 100,
    /// Data Sharing Agreement not found
    AgreementNotFound = 101,
    /// Agreement has expired
    AgreementExpired = 102,
    /// Insufficient permissions for requested access
    InsufficientPermissions = 103,
    /// Event type not covered by agreement
    EventTypeNotAllowed = 104,
    /// Selective disclosure proof invalid
    InvalidDisclosureProof = 105,
    /// Tamper-evidence chain broken
    TamperEvidenceViolation = 106,
    /// Compliance standard not supported
    UnsupportedStandard = 107,
    /// Access request not found
    AccessRequestNotFound = 108,
    /// Invalid signature on agreement
    InvalidAgreementSignature = 109,
    /// Regulator role not recognized
    InvalidRole = 110,
}

/// Helper struct for ISA 3000 compliance validation
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ISA3000ControlObjective {
    /// Control objective code (e.g., "CC6.1", "A1.1")
    pub code: Symbol,
    /// Description of the objective
    pub description: Bytes,
    /// Related audit evidence event types
    pub evidence_types: Vec<Symbol>,
    /// Whether objective requires continuous monitoring
    pub continuous_monitoring: bool,
}

/// Helper struct for SOC 2 compliance validation
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SOC2Criterion {
    /// Criterion code (e.g., "CC6.1", "A1.1")
    pub code: Symbol,
    /// Trust service principle (Security, Availability, Processing Integrity, Confidentiality, Privacy)
    pub principle: Bytes,
    /// Description of the criterion
    pub description: Bytes,
    /// Related audit evidence event types
    pub evidence_types: Vec<Symbol>,
}

/// Container for compliance rule sets
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComplianceRuleSet {
    /// Standard this ruleset applies to
    pub standard: ComplianceStandard,
    /// Version of the ruleset
    pub version: u32,
    /// Whether this ruleset is active
    pub active: bool,
    /// Rules encoded as opaque bytes (specific format per standard)
    pub rules_data: Bytes,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compliance_standard_ordering() {
        assert!(ComplianceStandard::ISA3000 < ComplianceStandard::SOC2);
        assert!(ComplianceStandard::SOC2 < ComplianceStandard::GDPR);
    }

    #[test]
    fn test_regulator_role_ordering() {
        assert!(RegulatorRole::Auditor < RegulatorRole::RegulatorOfficer);
        assert!(RegulatorRole::RegulatorOfficer < RegulatorRole::RegulatoryAdmin);
    }

    #[test]
    fn test_sensitivity_level_ordering() {
        assert!(SensitivityLevel::Public < SensitivityLevel::Internal);
        assert!(SensitivityLevel::Internal < SensitivityLevel::Confidential);
        assert!(SensitivityLevel::Confidential < SensitivityLevel::Restricted);
    }
}
