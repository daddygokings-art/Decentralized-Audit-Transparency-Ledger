/// # Records of Processing Activities (ROPA) Module
///
/// Comprehensive Record of Processing Activities (ROPA) management framework
/// implementing GDPR Article 30 requirements with automated record maintenance,
/// processing purpose tracking, data category classification, recipient management,
/// transfer tracking, retention policies, and security measure documentation.
///
/// ## Regulatory Framework
/// - **GDPR Article 30** — Records of processing activities
/// - **ISO 27701** — Privacy Information Management System
/// - **eIDAS** — Electronic identification and trust services

use soroban_sdk::{
    bytes, contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Bytes,
    BytesN, Env, Symbol, Vec,
};

// ── Error Codes ──────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ROPAError {
    /// Record not found
    RecordNotFound = 6100,
    /// Processing activity not registered
    ActivityNotRegistered = 6101,
    /// Data category not recognized
    DataCategoryNotRecognized = 6102,
    /// Recipient not authorized
    RecipientNotAuthorized = 6103,
    /// Retention policy invalid
    RetentionPolicyInvalid = 6104,
    /// Transfer mechanism missing
    TransferMechanismMissing = 6105,
    /// Security measure insufficient
    SecurityMeasureInsufficient = 6106,
    /// Record already exists
    RecordAlreadyExists = 6107,
    /// Update not permitted
    UpdateNotPermitted = 6108,
    /// Audit trail corrupted
    AuditTrailCorrupted = 6109,
    /// DPO consent required
    DPOConsentRequired = 6110,
    /// Processing purpose missing
    ProcessingPurposeMissing = 6111,
}

// ── Data Types ───────────────────────────────────────────────────────────

/// Processing purpose category
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum ProcessingPurpose {
    ContractPerformance = 1,
    LegalObligation = 2,
    VitalInterests = 3,
    PublicTask = 4,
    LegitimateInterest = 5,
    Consent = 6,
}

/// Data subject category
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum DataSubjectCategory {
    Customers = 1,
    Employees = 2,
    Suppliers = 3,
    Users = 4,
    Patients = 5,
    Students = 6,
    Citizens = 7,
}

/// Data category classification
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum DataCategory {
    IdentityData = 1,
    ContactData = 2,
    FinancialData = 3,
    HealthData = 4,
    BiometricData = 5,
    LocationData = 6,
    CommunicationData = 7,
    SpecialCategory = 8,
}

/// Recipient type
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

/// Recipient type
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum RecipientType {
    InternalTeam = 1,
    Processor = 2,
    Subprocessor = 3,
    Authority = 4,
    ThirdParty = 5,
    JointController = 6,
}

/// ROPA record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ROPARecord {
    pub id: BytesN<32>,
    pub controller: Address,
    pub dpo: Address,
    pub processing_name: Bytes,
    pub processing_purposes: Vec<u32>,
    pub data_categories: Vec<u32>,
    pub data_subjects_categories: Vec<u32>,
    pub recipients: Vec<RecipientInfo>,
    pub transfers_to_third_countries: Vec<TransferInfo>,
    pub retention_period_days: u32,
    pub security_measures: Vec<Bytes>,
    pub description: Bytes,
    pub created_at: u64,
    pub updated_at: u64,
    pub last_audited_at: u64,
    pub record_hash: BytesN<32>,
}

/// Recipient information
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipientInfo {
    pub recipient_type: u32,
    pub name: Bytes,
    pub address: Address,
    pub country: Bytes,
    pub purpose: Bytes,
    pub safeguards: Bytes,
}

/// Transfer information
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferInfo {
    pub country: Bytes,
    pub mechanism: u32,
    pub data_categories: Vec<u32>,
    pub frequency: Bytes,
    pub documentation: Bytes,
    pub transfer_hash: BytesN<32>,
}

/// Processing activity
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessingActivity {
    pub id: BytesN<32>,
    pub ropa_id: BytesN<32>,
    pub activity_name: Bytes,
    pub description: Bytes,
    pub legal_basis: u32,
    pub started_at: u64,
    pub is_active: bool,
    pub activity_hash: BytesN<32>,
}

// ── Storage Keys ─────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ROPAKey {
    Owner,
    ROPARecord(BytesN<32>),
    ROPAByController(Address),
    ProcessingActivity(BytesN<32>),
    ActivityByROPA(BytesN<32>),
    RecordCount,
    ActivityCount,
}

// ── Contract Implementation ──────────────────────────────────────────────

#[contract]
pub struct ROPAManager;

#[contractimpl]
impl ROPAManager {
    /// Initialize ROPA management module
    pub fn initialize(env: Env, owner: Address) {
        owner.require_auth();
        env.storage()
            .instance()
            .set(&ROPAKey::Owner, &owner);
        env.storage()
            .instance()
            .set(&ROPAKey::RecordCount, &0u32);
        env.storage()
            .instance()
            .set(&ROPAKey::ActivityCount, &0u32);
    }

    // ── ROPA Record Management ───────────────────────────────────────────

    pub fn create_record(
        env: Env,
        caller: Address,
        controller: Address,
        dpo: Address,
        processing_name: Bytes,
        processing_purposes: Vec<u32>,
        data_categories: Vec<u32>,
        data_subjects_categories: Vec<u32>,
        recipients: Vec<RecipientInfo>,
        transfers_to_third_countries: Vec<TransferInfo>,
        retention_period_days: u32,
        security_measures: Vec<Bytes>,
        description: Bytes,
    ) -> BytesN<32> {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        if processing_purposes.is_empty() {
            panic_with_error!(&env, ROPAError::ProcessingPurposeMissing);
        }

        let record_id = env.crypto().sha256(&processing_name.clone()).into();
        let now = env.ledger().timestamp();

        let record = ROPARecord {
            id: record_id.clone(),
            controller,
            dpo,
            processing_name: processing_name.clone(),
            processing_purposes,
            data_categories,
            data_subjects_categories,
            recipients,
            transfers_to_third_countries,
            retention_period_days,
            security_measures,
            description,
            created_at: now,
            updated_at: now,
            last_audited_at: 0,
            record_hash: env
                .crypto()
                .sha256(&Self::pack_record_data(&env, &processing_name, now))
                .into(),
        };

        env.storage()
            .instance()
            .set(&ROPAKey::ROPARecord(record_id.clone()), &record);
        env.storage()
            .instance()
            .set(&ROPAKey::ROPAByController(controller), &record_id);

        let count: u32 = env
            .storage()
            .instance()
            .get(&ROPAKey::RecordCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&ROPAKey::RecordCount, &(count + 1));

        env.events().publish(
            (Symbol::new(&env, "ropa"), Symbol::new(&env, "record_created")),
            (record_id.clone(), controller, processing_name),
        );

        record_id
    }

    pub fn get_record(env: Env, record_id: BytesN<32>) -> ROPARecord {
        env.storage()
            .instance()
            .get(&ROPAKey::ROPARecord(record_id))
            .unwrap_or_else(|| panic_with_error!(&env, ROPAError::RecordNotFound))
    }

    pub fn update_record(
        env: Env,
        caller: Address,
        record_id: BytesN<32>,
        security_measures: Vec<Bytes>,
        retention_period_days: u32,
        description: Bytes,
    ) -> ROPARecord {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let mut record = Self::get_record(env.clone(), record_id.clone());
        let now = env.ledger().timestamp();

        record.security_measures = security_measures;
        record.retention_period_days = retention_period_days;
        record.description = description;
        record.updated_at = now;
        record.record_hash = env
            .crypto()
            .sha256(&Self::pack_update_data(&env, &record_id, now))
            .into();

        env.storage()
            .instance()
            .set(&ROPAKey::ROPARecord(record_id.clone()), &record);

        env.events().publish(
            (Symbol::new(&env, "ropa"), Symbol::new(&env, "record_updated")),
            (record_id, now),
        );

        record
    }

    // ── Processing Activity Management ───────────────────────────────────

    pub fn register_activity(
        env: Env,
        caller: Address,
        ropa_id: BytesN<32>,
        activity_name: Bytes,
        description: Bytes,
        legal_basis: u32,
    ) -> BytesN<32> {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let _record = Self::get_record(env.clone(), ropa_id.clone());

        let activity_id = env.crypto().sha256(&activity_name.clone()).into();
        let now = env.ledger().timestamp();

        let activity = ProcessingActivity {
            id: activity_id.clone(),
            ropa_id: ropa_id.clone(),
            activity_name: activity_name.clone(),
            description,
            legal_basis,
            started_at: now,
            is_active: true,
            activity_hash: env
                .crypto()
                .sha256(&Self::pack_activity_data(&env, &activity_name, now))
                .into(),
        };

        env.storage()
            .instance()
            .set(&ROPAKey::ProcessingActivity(activity_id.clone()), &activity);

        let count: u32 = env
            .storage()
            .instance()
            .get(&ROPAKey::ActivityCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&ROPAKey::ActivityCount, &(count + 1));

        env.events().publish(
            (Symbol::new(&env, "ropa"), Symbol::new(&env, "activity_registered")),
            (activity_id.clone(), ropa_id, activity_name),
        );

        activity_id
    }

    pub fn get_activity(env: Env, activity_id: BytesN<32>) -> ProcessingActivity {
        env.storage()
            .instance()
            .get(&ROPAKey::ProcessingActivity(activity_id))
            .unwrap_or_else(|| panic_with_error!(&env, ROPAError::ActivityNotRegistered))
    }

    // ── Audit Management ─────────────────────────────────────────────────

    pub fn mark_audited(
        env: Env,
        caller: Address,
        record_id: BytesN<32>,
    ) -> ROPARecord {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let mut record = Self::get_record(env.clone(), record_id.clone());
        let now = env.ledger().timestamp();

        record.last_audited_at = now;
        record.updated_at = now;

        env.storage()
            .instance()
            .set(&ROPAKey::ROPARecord(record_id.clone()), &record);

        env.events().publish(
            (Symbol::new(&env, "ropa"), Symbol::new(&env, "audit_completed")),
            (record_id, now),
        );

        record
    }

    // ── Statistics ───────────────────────────────────────────────────────

    pub fn get_ropa_stats(env: Env) -> (u32, u32) {
        let records: u32 = env
            .storage()
            .instance()
            .get(&ROPAKey::RecordCount)
            .unwrap_or(0);
        let activities: u32 = env
            .storage()
            .instance()
            .get(&ROPAKey::ActivityCount)
            .unwrap_or(0);

        (records, activities)
    }

    // ── Private Helpers ──────────────────────────────────────────────────

    fn require_owner(env: &Env, caller: &Address) {
        let owner: Address = env
            .storage()
            .instance()
            .get(&ROPAKey::Owner)
            .unwrap();
        if caller != &owner {
            panic_with_error!(env, ROPAError::RecipientNotAuthorized);
        }
    }

    fn pack_record_data(env: &Env, name: &Bytes, timestamp: u64) -> Bytes {
        let mut data = Bytes::new(env);
        data.append(name);
        data.append(&Self::u64_to_bytes(env, timestamp));
        data
    }

    fn pack_update_data(env: &Env, record_id: &BytesN<32>, timestamp: u64) -> Bytes {
        let mut data = Bytes::new(env);
        data.append(&record_id.clone().into());
        data.append(&Self::u64_to_bytes(env, timestamp));
        data
    }

    fn pack_activity_data(env: &Env, name: &Bytes, timestamp: u64) -> Bytes {
        let mut data = Bytes::new(env);
        data.append(name);
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
