/// # Data Processing Agreements Module
///
/// Comprehensive Data Processing Agreement (DPA) management framework
/// implementing GDPR Article 28 requirements with subprocessor tracking,
/// renewal automation, audit rights management, and international data
/// transfer mechanisms (Standard Contractual Clauses and adequacy decisions).
///
/// ## Regulatory Framework
/// - **GDPR Article 28** — Processor must process personal data only on documented instructions
/// - **SCCs** — Standard Contractual Clauses for third-country transfers
/// - **Adequacy Decisions** — European Commission adequacy determinations
/// - **BCRs** — Binding Corporate Rules for intra-group transfers

use soroban_sdk::{
    bytes, contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Bytes,
    BytesN, Env, Symbol, Vec,
};

// ── Error Codes ──────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum DPAError {
    /// Agreement not found
    AgreementNotFound = 6000,
    /// Subprocessor not authorized
    SubprocessorNotAuthorized = 6001,
    /// Agreement expired
    AgreementExpired = 6002,
    /// Renewal window not open
    RenewalWindowNotOpen = 6003,
    /// Audit rights not granted
    AuditRightsNotGranted = 6004,
    /// Transfer mechanism invalid
    TransferMechanismInvalid = 6005,
    /// Insufficient transfer safeguards
    InsufficientSafeguards = 6006,
    /// Subprocessor limit exceeded
    SubprocessorLimitExceeded = 6007,
    /// Agreement already active
    AgreementAlreadyActive = 6008,
    /// Revocation not permitted
    RevocationNotPermitted = 6009,
    /// Inadequate security measures
    InadequateSecurityMeasures = 6010,
    /// Transfer documentation missing
    TransferDocumentationMissing = 6011,
    /// Subprocessor not in registry
    SubprocessorNotRegistered = 6012,
}

// ── Data Types ───────────────────────────────────────────────────────────

/// DPA status lifecycle
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum AgreementStatus {
    Draft = 0,
    Active = 1,
    UnderReview = 2,
    Expired = 3,
    Terminated = 4,
    Suspended = 5,
}

/// International transfer mechanism type
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum TransferMechanism {
    AdequacyDecision = 1,
    SCCs = 2,
    BCRs = 3,
    Derogation = 4,
    Certification = 5,
}

/// Subprocessor authorization status
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum AuthorizationStatus {
    Pending = 0,
    Authorized = 1,
    Restricted = 2,
    Revoked = 3,
    Expired = 4,
}

/// Data Processing Agreement
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataProcessingAgreement {
    pub id: BytesN<32>,
    pub controller: Address,
    pub processor: Address,
    pub agreement_ref: Bytes,
    pub processing_purposes: Vec<Bytes>,
    pub data_categories: Vec<Bytes>,
    pub data_subjects: Vec<Bytes>,
    pub security_measures: Vec<Bytes>,
    pub audit_rights_granted: bool,
    pub audit_notice_period_days: u32,
    pub status: u32,
    pub effective_date: u64,
    pub expiration_date: u64,
    pub renewal_auto: bool,
    pub renewal_notice_days: u32,
    pub created_at: u64,
    pub updated_at: u64,
    pub agreement_hash: BytesN<32>,
}

/// Subprocessor record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Subprocessor {
    pub id: BytesN<32>,
    pub agreement_id: BytesN<32>,
    pub name: Bytes,
    pub address: Address,
    pub country: Bytes,
    pub processing_purposes: Vec<Bytes>,
    pub authorization_status: u32,
    pub security_certifications: Vec<Bytes>,
    pub audit_last_date: u64,
    pub audit_next_date: u64,
    pub registered_at: u64,
    pub subprocessor_hash: BytesN<32>,
}

/// International transfer record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferRecord {
    pub id: BytesN<32>,
    pub agreement_id: BytesN<32>,
    pub mechanism: u32,
    pub destination_country: Bytes,
    pub data_categories: Vec<Bytes>,
    pub frequency: Bytes,
    pub supplementary_measures: Vec<Bytes>,
    pub transfer_impact_assessment: Bytes,
    pub documented_at: u64,
    pub reviewed_at: u64,
    pub transfer_hash: BytesN<32>,
}

/// DPA audit record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditRecord {
    pub id: BytesN<32>,
    pub agreement_id: BytesN<32>,
    pub auditor: Address,
    pub audit_type: Bytes,
    pub scope: Bytes,
    pub findings: Bytes,
    pub status: u32,
    pub scheduled_date: u64,
    pub completed_date: u64,
    pub report_hash: BytesN<32>,
    pub created_at: u64,
}

// ── Storage Keys ─────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DPAKey {
    Owner,
    Agreement(BytesN<32>),
    AgreementByRef(Bytes),
    Subprocessor(BytesN<32>),
    SubprocessorByAgreement(BytesN<32>),
    TransferRecord(BytesN<32>),
    TransferByAgreement(BytesN<32>),
    AuditRecord(BytesN<32>),
    AuditByAgreement(BytesN<32>),
    AgreementCount,
    SubprocessorCount,
    TransferCount,
    AuditCount,
}

// ── Contract Implementation ──────────────────────────────────────────────

#[contract]
pub struct DataProcessingAgreements;

#[contractimpl]
impl DataProcessingAgreements {
    /// Initialize DPA management module
    pub fn initialize(env: Env, owner: Address) {
        owner.require_auth();
        env.storage()
            .instance()
            .set(&DPAKey::Owner, &owner);
        env.storage()
            .instance()
            .set(&DPAKey::AgreementCount, &0u32);
        env.storage()
            .instance()
            .set(&DPAKey::SubprocessorCount, &0u32);
        env.storage()
            .instance()
            .set(&DPAKey::TransferCount, &0u32);
        env.storage()
            .instance()
            .set(&DPAKey::AuditCount, &0u32);
    }

    // ── Agreement Management ─────────────────────────────────────────────

    pub fn register_agreement(
        env: Env,
        caller: Address,
        controller: Address,
        processor: Address,
        agreement_ref: Bytes,
        processing_purposes: Vec<Bytes>,
        data_categories: Vec<Bytes>,
        data_subjects: Vec<Bytes>,
        security_measures: Vec<Bytes>,
        audit_rights_granted: bool,
        audit_notice_period_days: u32,
        effective_date: u64,
        expiration_date: u64,
        renewal_auto: bool,
        renewal_notice_days: u32,
    ) -> BytesN<32> {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let agreement_id = env.crypto().sha256(&agreement_ref.clone()).into();
        let now = env.ledger().timestamp();

        let agreement = DataProcessingAgreement {
            id: agreement_id.clone(),
            controller,
            processor,
            agreement_ref: agreement_ref.clone(),
            processing_purposes,
            data_categories,
            data_subjects,
            security_measures,
            audit_rights_granted,
            audit_notice_period_days,
            status: AgreementStatus::Active as u32,
            effective_date,
            expiration_date,
            renewal_auto,
            renewal_notice_days,
            created_at: now,
            updated_at: now,
            agreement_hash: env
                .crypto()
                .sha256(&Self::pack_agreement_data(&env, &agreement_ref, now))
                .into(),
        };

        env.storage()
            .instance()
            .set(&DPAKey::Agreement(agreement_id.clone()), &agreement);
        env.storage()
            .instance()
            .set(&DPAKey::AgreementByRef(agreement_ref), &agreement_id);

        let count: u32 = env
            .storage()
            .instance()
            .get(&DPAKey::AgreementCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DPAKey::AgreementCount, &(count + 1));

        env.events().publish(
            (Symbol::new(&env, "dpa"), Symbol::new(&env, "registered")),
            (agreement_id.clone(), controller, processor),
        );

        agreement_id
    }

    pub fn get_agreement(env: Env, agreement_id: BytesN<32>) -> DataProcessingAgreement {
        env.storage()
            .instance()
            .get(&DPAKey::Agreement(agreement_id))
            .unwrap_or_else(|| panic_with_error!(&env, DPAError::AgreementNotFound))
    }

    pub fn renew_agreement(
        env: Env,
        caller: Address,
        agreement_id: BytesN<32>,
        new_expiration_date: u64,
    ) -> DataProcessingAgreement {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let mut agreement = Self::get_agreement(env.clone(), agreement_id.clone());
        let now = env.ledger().timestamp();

        if agreement.status != AgreementStatus::Active as u32 {
            panic_with_error!(&env, DPAError::AgreementExpired);
        }

        if !agreement.renewal_auto {
            panic_with_error!(&env, DPAError::RenewalWindowNotOpen);
        }

        if new_expiration_date <= agreement.expiration_date {
            panic_with_error!(&env, DPAError::RenewalWindowNotOpen);
        }

        agreement.expiration_date = new_expiration_date;
        agreement.updated_at = now;
        agreement.agreement_hash = env
            .crypto()
            .sha256(&Self::pack_renewal_data(&env, &agreement_id, now))
            .into();

        env.storage()
            .instance()
            .set(&DPAKey::Agreement(agreement_id.clone()), &agreement);

        env.events().publish(
            (Symbol::new(&env, "dpa"), Symbol::new(&env, "renewed")),
            (agreement_id, new_expiration_date),
        );

        agreement
    }

    pub fn terminate_agreement(
        env: Env,
        caller: Address,
        agreement_id: BytesN<32>,
    ) -> DataProcessingAgreement {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let mut agreement = Self::get_agreement(env.clone(), agreement_id.clone());
        let now = env.ledger().timestamp();

        if agreement.status == AgreementStatus::Terminated as u32 {
            panic_with_error!(&env, DPAError::RevocationNotPermitted);
        }

        agreement.status = AgreementStatus::Terminated as u32;
        agreement.updated_at = now;

        env.storage()
            .instance()
            .set(&DPAKey::Agreement(agreement_id.clone()), &agreement);

        env.events().publish(
            (Symbol::new(&env, "dpa"), Symbol::new(&env, "terminated")),
            (agreement_id, now),
        );

        agreement
    }

    // ── Subprocessor Management ──────────────────────────────────────────

    pub fn register_subprocessor(
        env: Env,
        caller: Address,
        agreement_id: BytesN<32>,
        name: Bytes,
        processor_address: Address,
        country: Bytes,
        processing_purposes: Vec<Bytes>,
        security_certifications: Vec<Bytes>,
        audit_next_date: u64,
    ) -> BytesN<32> {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let _agreement = Self::get_agreement(env.clone(), agreement_id.clone());

        let subprocessor_id = env.crypto().sha256(&name.clone()).into();
        let now = env.ledger().timestamp();

        let subprocessor = Subprocessor {
            id: subprocessor_id.clone(),
            agreement_id: agreement_id.clone(),
            name: name.clone(),
            address: processor_address,
            country,
            processing_purposes,
            authorization_status: AuthorizationStatus::Pending as u32,
            security_certifications,
            audit_last_date: 0,
            audit_next_date,
            registered_at: now,
            subprocessor_hash: env
                .crypto()
                .sha256(&Self::pack_subprocessor_data(&env, &name, now))
                .into(),
        };

        env.storage()
            .instance()
            .set(&DPAKey::Subprocessor(subprocessor_id.clone()), &subprocessor);

        let count: u32 = env
            .storage()
            .instance()
            .get(&DPAKey::SubprocessorCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DPAKey::SubprocessorCount, &(count + 1));

        env.events().publish(
            (Symbol::new(&env, "dpa"), Symbol::new(&env, "subprocessor_registered")),
            (subprocessor_id.clone(), agreement_id, name),
        );

        subprocessor_id
    }

    pub fn authorize_subprocessor(
        env: Env,
        caller: Address,
        subprocessor_id: BytesN<32>,
    ) -> Subprocessor {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let mut subprocessor = Self::get_subprocessor(env.clone(), subprocessor_id.clone());
        subprocessor.authorization_status = AuthorizationStatus::Authorized as u32;

        env.storage()
            .instance()
            .set(&DPAKey::Subprocessor(subprocessor_id.clone()), &subprocessor);

        env.events().publish(
            (Symbol::new(&env, "dpa"), Symbol::new(&env, "subprocessor_authorized")),
            (subprocessor_id, subprocessor.address),
        );

        subprocessor
    }

    pub fn get_subprocessor(env: Env, subprocessor_id: BytesN<32>) -> Subprocessor {
        env.storage()
            .instance()
            .get(&DPAKey::Subprocessor(subprocessor_id))
            .unwrap_or_else(|| panic_with_error!(&env, DPAError::SubprocessorNotRegistered))
    }

    // ── Transfer Mechanism Management ────────────────────────────────────

    pub fn register_transfer(
        env: Env,
        caller: Address,
        agreement_id: BytesN<32>,
        mechanism: u32,
        destination_country: Bytes,
        data_categories: Vec<Bytes>,
        frequency: Bytes,
        supplementary_measures: Vec<Bytes>,
        transfer_impact_assessment: Bytes,
    ) -> BytesN<32> {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let _agreement = Self::get_agreement(env.clone(), agreement_id.clone());

        if mechanism != TransferMechanism::AdequacyDecision as u32
            && mechanism != TransferMechanism::SCCs as u32
            && mechanism != TransferMechanism::BCRs as u32
            && mechanism != TransferMechanism::Derogation as u32
            && mechanism != TransferMechanism::Certification as u32
        {
            panic_with_error!(&env, DPAError::TransferMechanismInvalid);
        }

        if supplementary_measures.is_empty() && mechanism != TransferMechanism::AdequacyDecision as u32 {
            panic_with_error!(&env, DPAError::InsufficientSafeguards);
        }

        let transfer_id = env.crypto().sha256(&destination_country.clone()).into();
        let now = env.ledger().timestamp();

        let transfer = TransferRecord {
            id: transfer_id.clone(),
            agreement_id: agreement_id.clone(),
            mechanism,
            destination_country: destination_country.clone(),
            data_categories,
            frequency,
            supplementary_measures,
            transfer_impact_assessment,
            documented_at: now,
            reviewed_at: 0,
            transfer_hash: env
                .crypto()
                .sha256(&Self::pack_transfer_data(&env, &destination_country, now))
                .into(),
        };

        env.storage()
            .instance()
            .set(&DPAKey::TransferRecord(transfer_id.clone()), &transfer);

        let count: u32 = env
            .storage()
            .instance()
            .get(&DPAKey::TransferCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DPAKey::TransferCount, &(count + 1));

        env.events().publish(
            (Symbol::new(&env, "dpa"), Symbol::new(&env, "transfer_registered")),
            (transfer_id.clone(), agreement_id, destination_country),
        );

        transfer_id
    }

    pub fn get_transfer(env: Env, transfer_id: BytesN<32>) -> TransferRecord {
        env.storage()
            .instance()
            .get(&DPAKey::TransferRecord(transfer_id))
            .unwrap_or_else(|| panic_with_error!(&env, DPAError::TransferDocumentationMissing))
    }

    // ── Audit Rights Management ──────────────────────────────────────────

    pub fn schedule_audit(
        env: Env,
        caller: Address,
        agreement_id: BytesN<32>,
        auditor: Address,
        audit_type: Bytes,
        scope: Bytes,
        scheduled_date: u64,
    ) -> BytesN<32> {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let agreement = Self::get_agreement(env.clone(), agreement_id.clone());

        if !agreement.audit_rights_granted {
            panic_with_error!(&env, DPAError::AuditRightsNotGranted);
        }

        let audit_id = env.crypto().sha256(&scope.clone()).into();
        let now = env.ledger().timestamp();

        let audit = AuditRecord {
            id: audit_id.clone(),
            agreement_id: agreement_id.clone(),
            auditor,
            audit_type,
            scope: scope.clone(),
            findings: Bytes::new(&env),
            status: 0,
            scheduled_date,
            completed_date: 0,
            report_hash: BytesN::from_array(&env, &[0u8; 32]),
            created_at: now,
        };

        env.storage()
            .instance()
            .set(&DPAKey::AuditRecord(audit_id.clone()), &audit);

        let count: u32 = env
            .storage()
            .instance()
            .get(&DPAKey::AuditCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DPAKey::AuditCount, &(count + 1));

        env.events().publish(
            (Symbol::new(&env, "dpa"), Symbol::new(&env, "audit_scheduled")),
            (audit_id.clone(), agreement_id, scheduled_date),
        );

        audit_id
    }

    pub fn complete_audit(
        env: Env,
        caller: Address,
        audit_id: BytesN<32>,
        findings: Bytes,
        report_hash: BytesN<32>,
    ) -> AuditRecord {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let mut audit = Self::get_audit(env.clone(), audit_id.clone());
        let now = env.ledger().timestamp();

        audit.findings = findings;
        audit.status = 1;
        audit.completed_date = now;
        audit.report_hash = report_hash;

        env.storage()
            .instance()
            .set(&DPAKey::AuditRecord(audit_id.clone()), &audit);

        env.events().publish(
            (Symbol::new(&env, "dpa"), Symbol::new(&env, "audit_completed")),
            (audit_id, audit.agreement_id, now),
        );

        audit
    }

    pub fn get_audit(env: Env, audit_id: BytesN<32>) -> AuditRecord {
        env.storage()
            .instance()
            .get(&DPAKey::AuditRecord(audit_id))
            .unwrap_or_else(|| panic_with_error!(&env, DPAError::AgreementNotFound))
    }

    // ── Statistics ───────────────────────────────────────────────────────

    pub fn get_dpa_stats(env: Env) -> (u32, u32, u32, u32) {
        let agreements: u32 = env
            .storage()
            .instance()
            .get(&DPAKey::AgreementCount)
            .unwrap_or(0);
        let subprocessors: u32 = env
            .storage()
            .instance()
            .get(&DPAKey::SubprocessorCount)
            .unwrap_or(0);
        let transfers: u32 = env
            .storage()
            .instance()
            .get(&DPAKey::TransferCount)
            .unwrap_or(0);
        let audits: u32 = env
            .storage()
            .instance()
            .get(&DPAKey::AuditCount)
            .unwrap_or(0);

        (agreements, subprocessors, transfers, audits)
    }

    // ── Private Helpers ──────────────────────────────────────────────────

    fn require_owner(env: &Env, caller: &Address) {
        let owner: Address = env
            .storage()
            .instance()
            .get(&DPAKey::Owner)
            .unwrap();
        if caller != &owner {
            panic_with_error!(env, DPAError::SubprocessorNotAuthorized);
        }
    }

    fn pack_agreement_data(env: &Env, ref_data: &Bytes, timestamp: u64) -> Bytes {
        let mut data = Bytes::new(env);
        data.append(ref_data);
        data.append(&Self::u64_to_bytes(env, timestamp));
        data
    }

    fn pack_renewal_data(env: &Env, agreement_id: &BytesN<32>, timestamp: u64) -> Bytes {
        let mut data = Bytes::new(env);
        data.append(&agreement_id.clone().into());
        data.append(&Self::u64_to_bytes(env, timestamp));
        data
    }

    fn pack_subprocessor_data(env: &Env, name: &Bytes, timestamp: u64) -> Bytes {
        let mut data = Bytes::new(env);
        data.append(name);
        data.append(&Self::u64_to_bytes(env, timestamp));
        data
    }

    fn pack_transfer_data(env: &Env, country: &Bytes, timestamp: u64) -> Bytes {
        let mut data = Bytes::new(env);
        data.append(country);
        data.append(&Self::u64_to_bytes(env, timestamp));
        data
    }

    fn u64_to_bytes(env: &Env, v: u64) -> Bytes {
        bytes!(
            env,
            [
                (v & 0xff) as u8,
                ((v >> 8) & 0xff) as u8,
                ((v >> 16) & 0xff) as u8,
                ((v >> 24) & 0xff) as u8,
                ((v >> 32) & 0xff) as u8,
                ((v >> 40) & 0xff) as u8,
                ((v >> 48) & 0xff) as u8,
                ((v >> 56) & 0xff) as u8,
            ]
        )
    }
}

#[cfg(test)]
mod tests;
