//! Data Sharing Agreement Framework for Regulators
//!
//! Manages DSAs between entities and regulators, including contract storage,
//! validation, permissioning, and access control for audit data sharing.

use soroban_sdk::{contracttype, BytesN, Address, Symbol, Bytes, Vec, Env};
use crate::regulator::{
    DataSharingAgreement, AccessRequest, ComplianceStandard, RegulatorRole, SensitivityLevel,
};

/// Storage key for DSAs indexed by agreement ID
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct DSAKey {
    pub agreement_id: BytesN<32>,
}

/// Storage key for access requests indexed by request ID
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct AccessRequestKey {
    pub request_id: BytesN<32>,
}

/// Storage key for regulator role assignments
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct RoleAssignmentKey {
    pub regulator: Address,
}

/// DSA status enumeration
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum DSAStatus {
    /// Agreement is being negotiated
    Draft = 0,
    /// Pending execution by both parties
    PendingExecution = 1,
    /// Both parties have executed the agreement
    Executed = 2,
    /// Agreement has been terminated
    Terminated = 3,
}

/// Access control decision for a data request
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AccessDecision {
    /// Access is permitted
    Approved = 0,
    /// Access is denied
    Rejected = 1,
    /// Access requires additional conditions
    Conditional = 2,
}

/// Signature validation result
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignatureValidation {
    /// Whether the signature is valid
    pub valid: bool,
    /// Recovered signer address (if valid)
    pub signer: Option<Address>,
    /// Error message if invalid
    pub error: Option<Bytes>,
}

/// DSA event log entry
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DSAEventLog {
    /// Event timestamp
    pub timestamp: u64,
    /// Event type (created, executed, terminated, etc.)
    pub event_type: Symbol,
    /// Actor performing the event
    pub actor: Address,
    /// Associated DSA ID
    pub agreement_id: BytesN<32>,
    /// Event details
    pub details: Bytes,
}

/// Builder for creating DSAs
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DSABuilder {
    pub data_provider: Address,
    pub regulator_address: Address,
    pub effective_ledger: u32,
    pub expiry_ledger: u32,
    pub standards: Vec<ComplianceStandard>,
    pub allowed_event_types: Vec<Symbol>,
    pub role: RegulatorRole,
    pub min_sensitivity: SensitivityLevel,
}

impl DSABuilder {
    /// Create a new DSA builder
    pub fn new(
        env: &Env,
        data_provider: Address,
        regulator_address: Address,
    ) -> Self {
        DSABuilder {
            data_provider,
            regulator_address,
            effective_ledger: 0,
            expiry_ledger: 0,
            standards: Vec::new(env),
            allowed_event_types: Vec::new(env),
            role: RegulatorRole::Auditor,
            min_sensitivity: SensitivityLevel::Public,
        }
    }

    /// Set the effective ledger
    pub fn with_effective_ledger(mut self, ledger: u32) -> Self {
        self.effective_ledger = ledger;
        self
    }

    /// Set the expiry ledger (0 = no expiry)
    pub fn with_expiry_ledger(mut self, ledger: u32) -> Self {
        self.expiry_ledger = ledger;
        self
    }

    /// Add a compliance standard
    pub fn add_standard(mut self, standard: ComplianceStandard) -> Self {
        self.standards.push_back(standard);
        self
    }

    /// Add an allowed event type
    pub fn add_event_type(mut self, event_type: Symbol) -> Self {
        self.allowed_event_types.push_back(event_type);
        self
    }

    /// Set the regulator role
    pub fn with_role(mut self, role: RegulatorRole) -> Self {
        self.role = role;
        self
    }

    /// Set minimum sensitivity level
    pub fn with_min_sensitivity(mut self, sensitivity: SensitivityLevel) -> Self {
        self.min_sensitivity = sensitivity;
        self
    }

    /// Build the DSA (without signatures)
    pub fn build(self) -> DataSharingAgreement {
        DataSharingAgreement {
            id: BytesN::<32>::from_array(&Env::default(), &[0u8; 32]),
            data_provider: self.data_provider,
            regulator_address: self.regulator_address,
            effective_ledger: self.effective_ledger,
            expiry_ledger: self.expiry_ledger,
            standards: self.standards,
            allowed_event_types: self.allowed_event_types,
            role: self.role,
            min_sensitivity: self.min_sensitivity,
            active: true,
            signature_provider: BytesN::<64>::from_array(&Env::default(), &[0u8; 64]),
            signature_regulator: BytesN::<64>::from_array(&Env::default(), &[0u8; 64]),
        }
    }
}

/// Access request builder
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessRequestBuilder {
    pub requester: Address,
    pub data_owner: Address,
    pub standard: ComplianceStandard,
    pub event_types: Vec<Symbol>,
    pub legal_basis: Bytes,
}

impl AccessRequestBuilder {
    /// Create a new access request builder
    pub fn new(
        env: &Env,
        requester: Address,
        data_owner: Address,
        standard: ComplianceStandard,
    ) -> Self {
        AccessRequestBuilder {
            requester,
            data_owner,
            standard,
            event_types: Vec::new(env),
            legal_basis: Bytes::new(env),
        }
    }

    /// Add an event type to the request
    pub fn add_event_type(mut self, event_type: Symbol) -> Self {
        self.event_types.push_back(event_type);
        self
    }

    /// Set the legal basis
    pub fn with_legal_basis(mut self, basis: Bytes) -> Self {
        self.legal_basis = basis;
        self
    }
}

/// Helper functions for DSA operations
pub struct DSAHelper;

impl DSAHelper {
    /// Calculate the DSA ID as a hash of the agreement components
    pub fn calculate_agreement_id(
        data_provider: &Address,
        regulator: &Address,
        timestamp: u64,
    ) -> BytesN<32> {
        // In production, use env.crypto_sha256() to hash the agreement data
        // For now, create a deterministic ID from the addresses and timestamp
        let mut result = [0u8; 32];
        
        // Simple hash: mix address bytes with timestamp
        for i in 0..8 {
            result[i] = ((timestamp >> (i * 8)) & 0xFF) as u8;
        }
        
        BytesN::<32>::from_array(&Env::default(), &result)
    }

    /// Verify if a DSA is currently active
    pub fn is_dsa_active(dsa: &DataSharingAgreement, current_ledger: u32) -> bool {
        dsa.active
            && current_ledger >= dsa.effective_ledger
            && (dsa.expiry_ledger == 0 || current_ledger <= dsa.expiry_ledger)
    }

    /// Check if an event type is allowed by a DSA
    pub fn is_event_type_allowed(dsa: &DataSharingAgreement, event_type: &Symbol) -> bool {
        if dsa.allowed_event_types.is_empty() {
            true // Empty list means all types allowed
        } else {
            dsa.allowed_event_types.iter().any(|t| t == *event_type)
        }
    }

    /// Check if a compliance standard is allowed by a DSA
    pub fn is_standard_allowed(dsa: &DataSharingAgreement, standard: &ComplianceStandard) -> bool {
        if dsa.standards.is_empty() {
            true // Empty list means all standards allowed
        } else {
            dsa.standards.iter().any(|s| s == *standard)
        }
    }

    /// Check if sensitivity level is within acceptable range
    pub fn is_sensitivity_allowed(
        dsa: &DataSharingAgreement,
        sensitivity: &SensitivityLevel,
    ) -> bool {
        sensitivity >= &dsa.min_sensitivity
    }

    /// Create a DSA event log entry
    pub fn create_event_log(
        env: &Env,
        event_type: Symbol,
        actor: Address,
        agreement_id: BytesN<32>,
        details: Bytes,
    ) -> DSAEventLog {
        DSAEventLog {
            timestamp: env.ledger().timestamp(),
            event_type,
            actor,
            agreement_id,
            details,
        }
    }

    /// Determine access decision based on DSA and request
    pub fn evaluate_access(
        dsa: &DataSharingAgreement,
        request: &AccessRequest,
        current_ledger: u32,
    ) -> AccessDecision {
        // Check if DSA is active
        if !Self::is_dsa_active(dsa, current_ledger) {
            return AccessDecision::Rejected;
        }

        // Check if standard is allowed
        if !Self::is_standard_allowed(dsa, &request.standard) {
            return AccessDecision::Rejected;
        }

        // Check if all requested event types are allowed
        for event_type in request.event_types.iter() {
            if !Self::is_event_type_allowed(dsa, &event_type) {
                return AccessDecision::Rejected;
            }
        }

        // Check if regulator has appropriate role
        if request.requester == dsa.regulator_address
            && (dsa.role == RegulatorRole::RegulatorOfficer
                || dsa.role == RegulatorRole::RegulatoryAdmin)
        {
            AccessDecision::Approved
        } else if request.requester == dsa.regulator_address
            && dsa.role == RegulatorRole::Auditor
        {
            // Auditor role has limited access
            AccessDecision::Conditional
        } else {
            AccessDecision::Rejected
        }
    }

    /// Verify DSA signatures (in production, would use cryptographic verification)
    pub fn verify_dsa_signatures(dsa: &DataSharingAgreement) -> bool {
        // In production, verify:
        // 1. signature_provider is valid signature by data_provider
        // 2. signature_regulator is valid signature by regulator_address
        // For now, basic validation
        dsa.signature_provider != BytesN::<64>::from_array(&Env::default(), &[0u8; 64])
            && dsa.signature_regulator != BytesN::<64>::from_array(&Env::default(), &[0u8; 64])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dsa_builder_creation() {
        let env = Env::default();
        let provider = Address::random(&env);
        let regulator = Address::random(&env);

        let builder = DSABuilder::new(&env, provider.clone(), regulator.clone());
        assert_eq!(builder.data_provider, provider);
        assert_eq!(builder.regulator_address, regulator);
        assert_eq!(builder.role, RegulatorRole::Auditor);
    }

    #[test]
    fn test_dsa_is_active_before_effective_ledger() {
        let env = Env::default();
        let provider = Address::random(&env);
        let regulator = Address::random(&env);

        let dsa = DSABuilder::new(&env, provider, regulator)
            .with_effective_ledger(100)
            .build();

        assert!(!DSAHelper::is_dsa_active(&dsa, 50));
    }

    #[test]
    fn test_dsa_is_active_after_expiry() {
        let env = Env::default();
        let provider = Address::random(&env);
        let regulator = Address::random(&env);

        let dsa = DSABuilder::new(&env, provider, regulator)
            .with_effective_ledger(10)
            .with_expiry_ledger(100)
            .build();

        assert!(!DSAHelper::is_dsa_active(&dsa, 150));
    }

    #[test]
    fn test_event_type_allowed_in_empty_list() {
        let env = Env::default();
        let provider = Address::random(&env);
        let regulator = Address::random(&env);

        let dsa = DSABuilder::new(&env, provider, regulator).build();

        assert!(DSAHelper::is_event_type_allowed(
            &dsa,
            &Symbol::new(&env, "payment")
        ));
    }
}
