//! Auditable privacy workflows for contract events.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Bytes,
    BytesN, Env, Symbol, Vec,
};

const NOTIFICATION_WINDOW: u64 = 72 * 60 * 60;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BreachStatus { Detected, Assessed, AuthorityNotified, SubjectsNotified, Closed }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BreachWorkflow {
    pub id: BytesN<32>, pub reporter: Address, pub detected_at: u64,
    pub notification_deadline: u64, pub severity: u32,
    pub affected_events: Vec<BytesN<32>>, pub assessment: Bytes,
    pub authority_notified_at: u64, pub subjects_notified_at: u64,
    pub status: BreachStatus, pub post_incident_review: Bytes,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PetRegistration {
    pub id: BytesN<32>, pub name: Symbol, pub technique: Symbol,
    pub use_case: Bytes, pub benchmark: Bytes, pub enabled: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TeeAttestation {
    pub event_id: BytesN<32>, pub platform: Symbol, pub measurement: BytesN<32>,
    pub attestation: Bytes, pub sealed_output_hash: BytesN<32>,
    pub verified_at: u64, pub verifier: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisclosurePolicy {
    pub event_id: BytesN<32>, pub commitment: BytesN<32>, pub allowed_fields: Vec<Symbol>,
    pub authorized_verifier: Address, pub expires_at: u64,
}

#[contracttype]
pub enum PrivacyKey {
    Owner, NextBreach, NextPet, Breach(BytesN<32>), Pet(BytesN<32>),
    Tee(BytesN<32>), Disclosure(BytesN<32>), BreachIds,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum PrivacyError { Unauthorized = 1, NotFound = 2, InvalidSeverity = 3, AlreadyNotified = 4, InvalidDeadline = 5 }

#[contract]
pub struct ContractEventPrivacy;

#[contractimpl]
impl ContractEventPrivacy {
    pub fn initialize(env: Env, owner: Address) {
        owner.require_auth();
        if env.storage().instance().has(&PrivacyKey::Owner) { panic_with_error!(&env, PrivacyError::Unauthorized); }
        env.storage().instance().set(&PrivacyKey::Owner, &owner);
        env.storage().instance().set(&PrivacyKey::NextBreach, &1u32);
        env.storage().instance().set(&PrivacyKey::NextPet, &1u32);
    }

    pub fn detect_breach(env: Env, reporter: Address, severity: u32, affected_events: Vec<BytesN<32>>) -> BreachWorkflow {
        reporter.require_auth();
        if severity > 4 { panic_with_error!(&env, PrivacyError::InvalidSeverity); }
        let sequence = Self::next(&env, &PrivacyKey::NextBreach);
        let detected_at = env.ledger().timestamp();
        let record = BreachWorkflow {
            id: Self::id(&env, sequence), reporter, detected_at,
            notification_deadline: detected_at + NOTIFICATION_WINDOW, severity,
            affected_events, assessment: Bytes::new(&env), authority_notified_at: 0,
            subjects_notified_at: 0, status: BreachStatus::Detected,
            post_incident_review: Bytes::new(&env),
        };
        Self::save_breach(&env, record.clone()); record
    }

    pub fn assess_breach(env: Env, caller: Address, id: BytesN<32>, assessment: Bytes) {
        Self::owner(&env, &caller); let mut record = Self::breach(&env, id.clone());
        record.assessment = assessment; record.status = BreachStatus::Assessed; Self::save_breach(&env, record);
    }

    pub fn notify_authority(env: Env, caller: Address, id: BytesN<32>) {
        Self::owner(&env, &caller); let mut record = Self::breach(&env, id.clone());
        if record.authority_notified_at != 0 { panic_with_error!(&env, PrivacyError::AlreadyNotified); }
        record.authority_notified_at = env.ledger().timestamp(); record.status = BreachStatus::AuthorityNotified; Self::save_breach(&env, record);
    }

    pub fn notify_subjects(env: Env, caller: Address, id: BytesN<32>) {
        Self::owner(&env, &caller); let mut record = Self::breach(&env, id.clone());
        if record.subjects_notified_at != 0 { panic_with_error!(&env, PrivacyError::AlreadyNotified); }
        record.subjects_notified_at = env.ledger().timestamp(); record.status = BreachStatus::SubjectsNotified; Self::save_breach(&env, record);
    }

    pub fn complete_review(env: Env, caller: Address, id: BytesN<32>, review: Bytes) {
        Self::owner(&env, &caller); let mut record = Self::breach(&env, id.clone());
        record.post_incident_review = review; record.status = BreachStatus::Closed; Self::save_breach(&env, record);
    }

    pub fn get_breach(env: Env, id: BytesN<32>) -> BreachWorkflow { Self::breach(&env, id) }

    pub fn register_pet(env: Env, caller: Address, name: Symbol, technique: Symbol, use_case: Bytes, benchmark: Bytes) -> PetRegistration {
        Self::owner(&env, &caller); let sequence = Self::next(&env, &PrivacyKey::NextPet);
        let record = PetRegistration { id: Self::id(&env, sequence), name, technique, use_case, benchmark, enabled: true };
        env.storage().instance().set(&PrivacyKey::Pet(record.id.clone()), &record); record
    }

    pub fn record_tee_attestation(env: Env, verifier: Address, event_id: BytesN<32>, platform: Symbol, measurement: BytesN<32>, attestation: Bytes, sealed_output_hash: BytesN<32>) -> TeeAttestation {
        verifier.require_auth(); let record = TeeAttestation { event_id: event_id.clone(), platform, measurement, attestation, sealed_output_hash, verified_at: env.ledger().timestamp(), verifier };
        env.storage().instance().set(&PrivacyKey::Tee(event_id), &record); record
    }

    pub fn set_disclosure_policy(env: Env, caller: Address, event_id: BytesN<32>, commitment: BytesN<32>, allowed_fields: Vec<Symbol>, authorized_verifier: Address, expires_at: u64) -> DisclosurePolicy {
        Self::owner(&env, &caller);
        if expires_at <= env.ledger().timestamp() { panic_with_error!(&env, PrivacyError::InvalidDeadline); }
        let policy = DisclosurePolicy { event_id: event_id.clone(), commitment, allowed_fields, authorized_verifier, expires_at };
        env.storage().instance().set(&PrivacyKey::Disclosure(event_id), &policy); policy
    }

    fn owner(env: &Env, caller: &Address) {
        caller.require_auth();
        let owner: Address = env.storage().instance().get(&PrivacyKey::Owner).unwrap_or_else(|| panic_with_error!(&env, PrivacyError::Unauthorized));
        if owner != *caller { panic_with_error!(env, PrivacyError::Unauthorized); }
    }
    fn next(env: &Env, key: &PrivacyKey) -> u32 { let value: u32 = env.storage().instance().get(key).unwrap_or(1); env.storage().instance().set(key, &(value + 1)); value }
    fn id(env: &Env, value: u32) -> BytesN<32> { let mut bytes = [0u8; 32]; bytes[..4].copy_from_slice(&value.to_le_bytes()); BytesN::from_array(env, &bytes) }
    fn save_breach(env: &Env, record: BreachWorkflow) {
        env.storage().instance().set(&PrivacyKey::Breach(record.id.clone()), &record);
        let mut ids: Vec<BytesN<32>> = env.storage().instance().get(&PrivacyKey::BreachIds).unwrap_or_else(|| Vec::new(env));
        if !ids.iter().any(|id| id == record.id) { ids.push_back(record.id); }
        env.storage().instance().set(&PrivacyKey::BreachIds, &ids);
    }
    fn breach(env: &Env, id: BytesN<32>) -> BreachWorkflow { env.storage().instance().get(&PrivacyKey::Breach(id)).unwrap_or_else(|| panic_with_error!(env, PrivacyError::NotFound)) }
}