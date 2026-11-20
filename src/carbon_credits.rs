#![allow(dead_code)]

use crate::rbac::{RbacManager, Role};
use soroban_sdk::{
    contracterror, contracttype, panic_with_error, vec, Address, Bytes, BytesN, Env, String, Symbol,
    Vec,
};

/// Carbon credit tracking error types
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum CarbonCreditError {
    /// Credit not found in registry
    CreditNotFound = 3001,
    /// Invalid carbon amount
    InvalidCarbonAmount = 3002,
    /// Renewable energy verification failed
    VerificationFailed = 3003,
    /// Credit already retired
    AlreadyRetired = 3004,
    /// Credit is too old or expired
    CreditExpired = 3005,
    /// Unauthorized access or modification
    UnauthorizedAccess = 3006,
    /// Invalid offset calculation
    InvalidOffsetCalculation = 3007,
    /// Registry not found
    RegistryNotFound = 3008,
    /// Compliance standard not recognized
    UnknownStandard = 3009,
    /// Insufficient credits to retire
    InsufficientCredits = 3010,
    /// Invalid tokenization
    InvalidTokenization = 3011,
    /// Transfer failed
    TransferFailed = 3012,
    /// A derived identifier collided with an existing entry
    IdentifierCollision = 3013,
}

/// Carbon credit status
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreditStatus {
    Issued,      // Just issued
    Active,      // Active and tradeable
    Retired,     // Permanently retired
    Disputed,    // Under dispute
    Expired,     // No longer valid
}

/// Tamper-evident audit trail events for carbon credit state mutations.
///
/// `#[contracttype]` enums only support tuple variants, so each variant's
/// fields are positional; the field order is documented per variant.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CarbonCreditEvent {
    /// `(credit_id, issuer, carbon_tonnes)`
    CreditIssued(BytesN<32>, Address, u32),
    /// `(credit_id, verifier, energy_mwh)`
    CreditVerified(BytesN<32>, Address, u32),
    /// `(credit_id, token_owner, total_tokens)`
    CreditTokenized(BytesN<32>, Address, u128),
    /// `(credit_id, caller, carbon_tonnes)`
    CreditRetired(BytesN<32>, Address, u32),
    /// `(credit_id, from, to)`
    CreditTransferred(BytesN<32>, Address, Address),
    /// `(credit_id, auditor, measured_energy, approved)`
    CreditAudited(BytesN<32>, Address, u32, bool),
    /// `(credit_id, caller, registry_id)`
    CreditRegistered(BytesN<32>, Address, Bytes),
    /// `(registry_id, verified_by)`
    RegistryUpdated(Bytes, Address),
    /// `(credit_id, caller, standard)`
    CreditLinkedToStandard(BytesN<32>, Address, u32),
}

/// Emit a structured, tamper-evident audit event for a carbon credit mutation.
fn emit_carbon_credit_event(env: &Env, event: CarbonCreditEvent) {
    env.events().publish(
        (Symbol::new(env, "carbon"), Symbol::new(env, "credit")),
        event,
    );
}

/// Sentinel identifier used for "not tokenized" / "rejected" results.
fn zero_id(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[0u8; 32])
}

/// Derive a unique, deterministic record identifier for a verification or
/// audit record.
///
/// Binds a domain tag, the credit, the acting validator, the ledger sequence
/// and timestamp, and the record's position in its log. Two validators
/// attesting to the same credit in the same ledger therefore always receive
/// distinct identifiers, and the same inputs always reproduce the same id.
fn derive_record_id(
    env: &Env,
    domain: &[u8],
    credit_id: &BytesN<32>,
    actor: &Address,
    position: u32,
) -> BytesN<32> {
    let mut preimage = Bytes::from_slice(env, domain);
    preimage.append(&Bytes::from_array(env, &credit_id.to_array()));
    preimage.append(&actor.to_string().to_bytes());
    preimage.extend_from_slice(&env.ledger().sequence().to_le_bytes());
    preimage.extend_from_slice(&env.ledger().timestamp().to_le_bytes());
    preimage.extend_from_slice(&position.to_le_bytes());
    env.crypto().sha256(&preimage).to_bytes()
}

/// Renewable energy source types
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RenewableEnergyType {
    Solar,
    Wind,
    Hydro,
    Geothermal,
    Biomass,
    TidalWave,
    OceanThermal,
}

/// Carbon credit verification standard
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ComplianceStandard {
    Vcs,            // Verified Carbon Standard
    Gold,           // Gold Standard
    Cdm,            // Clean Development Mechanism
    Car,            // Climate Action Reserve
    Ace,            // American Carbon Exchange
    Custom,         // Custom standard
}

/// Renewable energy source information
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenewableEnergySource {
    pub source_type: RenewableEnergyType,    // Type of renewable energy
    pub facility_id: Bytes,                 // Facility identifier
    pub location: Bytes,                    // Geographic location
    pub capacity_mw: u32,                   // Capacity in MW
    pub energy_generated_mwh: u32,          // Energy generated in MWh
    pub verification_date: u64,             // When verified
    pub certifications: Vec<Bytes>,         // Applicable certifications
}

/// Carbon offset information
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Offset {
    pub offset_type: Symbol,                // Type of offset (reforestation, methane capture, etc.)
    pub project_id: Bytes,                  // Offset project ID
    pub tonnes_co2e: u32,                   // Tonnes CO2e offset
    pub project_location: Bytes,            // Where offset occurred
    pub verification_body: Address,         // Third-party verifier
    pub verification_date: u64,             // When verified
    pub expiration_date: u64,               // When offset expires
}

/// Carbon credit tokenization information
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tokenization {
    pub token_id: BytesN<32>,               // Unique token ID
    pub total_tokens: u128,                 // Total tokens issued
    pub tokens_retired: u128,               // Tokens retired
    pub token_owner: Address,               // Current owner
    pub market_value: u32,                  // USD value per token
    pub tradeable: bool,                    // Can be traded
}

/// Registry entry for a carbon credit
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryEntry {
    pub registry_id: Bytes,                 // Registry identifier
    pub registry_name: Bytes,               // Name of registry
    pub registry_url: Bytes,                // URL for verification
    pub issuance_date: u64,                 // When registered
    pub verified_by: Address,               // Who verified
    pub compliance_standard: ComplianceStandard,
}

/// Sustainability claim for verification
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SustainabilityClaim {
    pub claim_id: Bytes,                    // Claim identifier
    pub claimant: Address,                  // Who makes the claim
    pub claim_type: Symbol,                 // Type of claim (carbon_neutral, zero_waste, etc.)
    pub claim_description: Bytes,           // Description
    pub claimed_reduction: u32,             // kg CO2e reduction claimed
    pub supporting_evidence: Vec<Bytes>,    // Links to evidence
    pub claim_date: u64,                    // When claimed
}

/// Verification record for audits
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationRecord {
    pub verification_id: BytesN<32>,        // Unique verification ID
    pub auditor: Address,                   // Auditor address
    pub audit_date: u64,                    // When audited
    pub verified_amount: u32,               // kg CO2e verified
    pub issues_found: Vec<Bytes>,           // Any issues
    pub approved: bool,                     // Audit approved
    pub audit_notes: Bytes,                 // Audit notes
}

/// Main carbon credit structure
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CarbonCredit {
    pub credit_id: BytesN<32>,              // Unique credit ID
    pub issuer: Address,                    // Who issued the credit
    pub carbon_tonnes: u32,                 // Tonnes of CO2e
    pub renewable_source: RenewableEnergySource, // Renewable energy source
    pub offset: Offset,                     // Carbon offset info
    pub status: CreditStatus,               // Current status
    pub creation_date: u64,                 // When created
    pub retirement_date: Option<u64>,       // When retired (if applicable)
    pub tokenization: Tokenization, // Tokenization record (sentinel = not tokenized)
    pub registry: RegistryEntry,            // Registry information
    pub verification_records: Vec<VerificationRecord>, // Audit trail
    pub version: u32,                       // Version number
}

/// Portfolio status for tracking
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortfolioStatus {
    pub holder: Address,                    // Portfolio owner
    pub total_credits: u32,                 // Total credits held
    pub active_credits: u32,                // Active (non-retired)
    pub retired_credits: u32,               // Permanently retired
    pub total_co2e_retired: u32,            // Total CO2e retired
    pub portfolio_value_usd: u32,           // USD value
    pub last_updated: u64,                  // Last update
}

/// Carbon reduction report
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CarbonReductionReport {
    pub reporting_period: (u64, u64),       // Start and end dates
    pub total_verified_reduction: u32,      // kg CO2e verified reduced
    pub renewable_energy_mwh: u32,          // MWh from renewables
    pub offsets_purchased: u32,             // Offsets purchased
    pub credits_retired: u32,               // Credits retired
    pub facilities_audited: u32,            // Facilities audited
    pub compliance_rate: u32,               // % compliant
}

// ─────────────────────────────────────────────────────────────────────────────
// Storage Keys
// ─────────────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum CarbonCreditKey {
    /// Carbon credit by ID
    CarbonCredit(BytesN<32>),
    /// Credits by issuer
    IssuerCredits(Address),
    /// Credits by holder
    HolderCredits(Address),
    /// Token ID to credit
    TokenToCreditMapping(BytesN<32>),
    /// Registry entry
    RegistryEntry(Bytes),
    /// Verification records for credit
    VerificationRecords(BytesN<32>),
    /// Portfolio status
    PortfolioStatus(Address),
    /// Compliance standard info
    ComplianceStandardInfo(Symbol),
    /// Global credit counter
    CreditCounter,
    /// Retired credits count
    RetiredCount,
    /// Total CO2e retired
    TotalCo2eRetired,
    /// Marker that `verifier` has already attested to `credit_id`
    /// (one attestation per validator per credit).
    Attestation(BytesN<32>, Address),
}

// ─────────────────────────────────────────────────────────────────────────────
// Core Functions
// ─────────────────────────────────────────────────────────────────────────────

/// Issue a new carbon credit
pub fn issue_carbon_credit(
    env: &Env,
    issuer: Address,
    carbon_tonnes: u32,
    renewable_source: RenewableEnergySource,
    offset: Offset,
    registry: RegistryEntry,
    _standard: ComplianceStandard,
) -> BytesN<32> {
    issuer.require_auth();
    if RbacManager::require_role_min(env, &issuer, Role::Submitter).is_err() {
        return zero_id(env);
    }

    // The global issuance counter acts as a monotonic nonce: two issuances by
    // the same issuer, for the same tonnage, inside the same ledger would
    // otherwise hash to the same id and silently overwrite each other.
    let count: u32 = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::CreditCounter)
        .unwrap_or(0);

    // Deterministic binary preimage (strkey bytes + LE numerics): `format!` and
    // `Display for soroban_sdk::String` are unavailable in the wasm contract build.
    let mut credit_preimage = Bytes::from_slice(env, &carbon_tonnes.to_le_bytes());
    credit_preimage.append(&issuer.to_string().to_bytes());
    credit_preimage.extend_from_slice(&env.ledger().timestamp().to_le_bytes());
    credit_preimage.extend_from_slice(&count.to_le_bytes());
    let credit_id = env.crypto().sha256(&credit_preimage).to_bytes();

    // Defence in depth: never overwrite an existing credit.
    if env
        .storage()
        .persistent()
        .has(&CarbonCreditKey::CarbonCredit(credit_id.clone()))
    {
        panic_with_error!(env, CarbonCreditError::IdentifierCollision);
    }

    let credit = CarbonCredit {
        credit_id: credit_id.clone(),
        issuer: issuer.clone(),
        carbon_tonnes,
        renewable_source,
        offset,
        status: CreditStatus::Issued,
        creation_date: env.ledger().timestamp(),
        retirement_date: None,
        tokenization: Tokenization {
            token_id: zero_id(env),
            total_tokens: 0,
            tokens_retired: 0,
            token_owner: Address::from_string(&String::from_str(env, crate::NULL_ACCOUNT)),
            market_value: 0,
            tradeable: false,
        },
        registry,
        verification_records: Vec::new(env),
        version: 1,
    };

    // Store credit
    env.storage()
        .persistent()
        .set(&CarbonCreditKey::CarbonCredit(credit_id.clone()), &credit);

    // Add to issuer index
    let mut issuer_credits: Vec<BytesN<32>> = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::IssuerCredits(issuer.clone()))
        .unwrap_or_else(|| Vec::new(env));
    issuer_credits.push_back(credit_id.clone());
    env.storage()
        .persistent()
        .set(&CarbonCreditKey::IssuerCredits(issuer.clone()), &issuer_credits);

    // The issuer is the initial holder of the credit.
    let mut holder_credits: Vec<BytesN<32>> = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::HolderCredits(issuer.clone()))
        .unwrap_or_else(|| Vec::new(env));
    holder_credits.push_back(credit_id.clone());
    env.storage()
        .persistent()
        .set(&CarbonCreditKey::HolderCredits(issuer.clone()), &holder_credits);

    // Increment counter
    env.storage()
        .persistent()
        .set(&CarbonCreditKey::CreditCounter, &(count + 1));

    emit_carbon_credit_event(
        env,
        CarbonCreditEvent::CreditIssued(credit_id.clone(), issuer.clone(), carbon_tonnes),
    );

    credit_id
}

/// Verify renewable energy usage
pub fn verify_renewable_energy(
    env: &Env,
    credit_id: BytesN<32>,
    verifier: Address,
    energy_mwh: u32,
) -> bool {
    verifier.require_auth();
    if RbacManager::require_role_min(env, &verifier, Role::Auditor).is_err() {
        return false;
    }

    let mut credit: CarbonCredit = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::CarbonCredit(credit_id.clone()))
        .unwrap_or_else(|| panic!("Credit not found"));

    // Only freshly issued or already active credits accept attestations. A
    // late attestation must never resurrect a retired, expired, or disputed
    // credit back to `Active`.
    if credit.status != CreditStatus::Issued && credit.status != CreditStatus::Active {
        return false;
    }

    // Verify energy generation matches
    if credit.renewable_source.energy_generated_mwh != energy_mwh {
        return false;
    }

    // One attestation per validator per credit: a duplicate or replayed
    // submission is rejected without touching state.
    let attestation_key = CarbonCreditKey::Attestation(credit_id.clone(), verifier.clone());
    if env.storage().persistent().has(&attestation_key) {
        return false;
    }

    // Create verification record
    let verification = VerificationRecord {
        verification_id: derive_record_id(
            env,
            b"VERIFY",
            &credit_id,
            &verifier,
            credit.verification_records.len(),
        ),
        auditor: verifier.clone(),
        audit_date: env.ledger().timestamp(),
        verified_amount: credit.carbon_tonnes,
        issues_found: Vec::new(env),
        approved: true,
        audit_notes: Bytes::from_slice(env, b"Renewable energy verified"),
    };

    credit.verification_records.push_back(verification);
    credit.status = CreditStatus::Active;
    credit.version += 1;

    env.storage()
        .persistent()
        .set(&CarbonCreditKey::CarbonCredit(credit_id.clone()), &credit);
    env.storage().persistent().set(&attestation_key, &true);

    emit_carbon_credit_event(
        env,
        CarbonCreditEvent::CreditVerified(credit_id, verifier, energy_mwh),
    );

    true
}

/// Calculate carbon offset based on renewable energy
pub fn calculate_offset(_env: &Env, energy_mwh: u32) -> u32 {
    // Typical calculation: 1 MWh ≈ 0.5 tonnes CO2e offset
    energy_mwh / 2
}

/// Tokenize a carbon credit
pub fn tokenize_credit(
    env: &Env,
    credit_id: BytesN<32>,
    token_owner: Address,
    tokens_to_issue: u128,
    market_value: u32,
) -> BytesN<32> {
    token_owner.require_auth();
    if RbacManager::require_role_min(env, &token_owner, Role::Submitter).is_err() {
        return zero_id(env);
    }

    let mut credit: CarbonCredit = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::CarbonCredit(credit_id.clone()))
        .unwrap_or_else(|| panic!("Credit not found"));

    // A credit is tokenized at most once, and never after retirement.
    if credit.tokenization.token_id != zero_id(env) || credit.status == CreditStatus::Retired {
        return zero_id(env);
    }

    // Bind the credit id so one owner tokenizing several credits in the same
    // ledger receives distinct token ids.
    let mut token_preimage = Bytes::from_slice(env, b"TOKEN");
    token_preimage.append(&Bytes::from_array(env, &credit_id.to_array()));
    token_preimage.append(&token_owner.to_string().to_bytes());
    token_preimage.extend_from_slice(&env.ledger().timestamp().to_le_bytes());
    let token_id = env.crypto().sha256(&token_preimage).to_bytes();
    if env
        .storage()
        .persistent()
        .has(&CarbonCreditKey::TokenToCreditMapping(token_id.clone()))
    {
        panic_with_error!(env, CarbonCreditError::IdentifierCollision);
    }

    let tokenization = Tokenization {
        token_id: token_id.clone(),
        total_tokens: tokens_to_issue,
        tokens_retired: 0,
        token_owner: token_owner.clone(),
        market_value,
        tradeable: true,
    };

    credit.tokenization = tokenization;
    credit.version += 1;

    env.storage()
        .persistent()
        .set(&CarbonCreditKey::CarbonCredit(credit_id.clone()), &credit);

    env.storage()
        .persistent()
        .set(&CarbonCreditKey::TokenToCreditMapping(token_id.clone()), &credit_id);

    emit_carbon_credit_event(
        env,
        CarbonCreditEvent::CreditTokenized(credit_id, token_owner, tokens_to_issue),
    );

    token_id
}

/// Retire a carbon credit
pub fn retire_credit(
    env: &Env,
    caller: Address,
    credit_id: BytesN<32>,
    _retire_reason: Bytes,
) -> bool {
    caller.require_auth();
    if RbacManager::require_role_min(env, &caller, Role::Submitter).is_err() {
        return false;
    }

    let mut credit: CarbonCredit = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::CarbonCredit(credit_id.clone()))
        .unwrap_or_else(|| panic!("Credit not found"));

    if credit.status == CreditStatus::Retired {
        return false; // Already retired
    }

    let carbon_tonnes = credit.carbon_tonnes;
    credit.status = CreditStatus::Retired;
    credit.retirement_date = Some(env.ledger().timestamp());
    credit.version += 1;

    env.storage()
        .persistent()
        .set(&CarbonCreditKey::CarbonCredit(credit_id.clone()), &credit);

    // Update global retired count
    let retired: u32 = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::RetiredCount)
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&CarbonCreditKey::RetiredCount, &(retired + 1));

    // Update total CO2e retired
    let total_co2e: u32 = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::TotalCo2eRetired)
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(
            &CarbonCreditKey::TotalCo2eRetired,
            &(total_co2e + carbon_tonnes),
        );

    emit_carbon_credit_event(
        env,
        CarbonCreditEvent::CreditRetired(credit_id, caller, carbon_tonnes),
    );

    true
}

/// Check retirement status
pub fn check_retirement_status(env: &Env, credit_id: BytesN<32>) -> bool {
    let credit: CarbonCredit = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::CarbonCredit(credit_id))
        .unwrap_or_else(|| panic!("Credit not found"));

    credit.status == CreditStatus::Retired
}

/// Transfer credit to new holder
pub fn transfer_credit(
    env: &Env,
    credit_id: BytesN<32>,
    from: Address,
    to: Address,
) -> bool {
    from.require_auth();
    if RbacManager::require_role_min(env, &from, Role::Submitter).is_err() {
        return false;
    }

    let mut credit: CarbonCredit = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::CarbonCredit(credit_id.clone()))
        .unwrap_or_else(|| panic!("Credit not found"));

    if credit.status == CreditStatus::Retired {
        return false; // Can't transfer retired credits
    }

    // Update tokenization owner if applicable
    if credit.tokenization.token_id != zero_id(env) {
        credit.tokenization.token_owner = to.clone();
    }
    credit.version += 1;

    env.storage()
        .persistent()
        .set(&CarbonCreditKey::CarbonCredit(credit_id.clone()), &credit);

    // Update holder index
    let mut to_credits: Vec<BytesN<32>> = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::HolderCredits(to.clone()))
        .unwrap_or_else(|| Vec::new(env));
    to_credits.push_back(credit_id.clone());
    env.storage()
        .persistent()
        .set(&CarbonCreditKey::HolderCredits(to.clone()), &to_credits);

    emit_carbon_credit_event(
        env,
        CarbonCreditEvent::CreditTransferred(credit_id, from, to),
    );

    true
}

/// Verify sustainability claim
pub fn verify_sustainability_claim(
    _env: &Env,
    claim: SustainabilityClaim,
    verifier: Address,
) -> bool {
    verifier.require_auth();

    // Verify claim has supporting evidence
    if claim.supporting_evidence.is_empty() {
        return false;
    }

    // Verify claimed reduction is reasonable
    if claim.claimed_reduction == 0 || claim.claimed_reduction > 1_000_000 {
        return false;
    }

    true
}

/// Audit renewable energy usage
pub fn audit_renewable_usage(
    env: &Env,
    credit_id: BytesN<32>,
    auditor: Address,
    measured_energy: u32,
) -> VerificationRecord {
    auditor.require_auth();
    if RbacManager::require_role_min(env, &auditor, Role::Auditor).is_err() {
        // Return a rejected record when the auditor lacks the required role.
        return VerificationRecord {
            verification_id: zero_id(env),
            auditor,
            audit_date: env.ledger().timestamp(),
            verified_amount: 0,
            issues_found: vec![env, Bytes::from_slice(env, b"insufficient role")],
            approved: false,
            audit_notes: Bytes::from_slice(env, b"Unauthorized auditor"),
        };
    }

    let credit: CarbonCredit = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::CarbonCredit(credit_id.clone()))
        .unwrap_or_else(|| panic!("Credit not found"));

    let mut issues = Vec::new(env);
    let mut approved = true;

    // Check if measured energy matches recorded (`abs_diff` cannot overflow,
    // unlike the signed subtraction of two u32 values cast to i32).
    if credit
        .renewable_source
        .energy_generated_mwh
        .abs_diff(measured_energy)
        > 100
    {
        issues.push_back(Bytes::from_slice(env, b"Energy mismatch"));
        approved = false;
    }

    let mut records: Vec<VerificationRecord> = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::VerificationRecords(credit_id.clone()))
        .unwrap_or_else(|| Vec::new(env));

    let record = VerificationRecord {
        verification_id: derive_record_id(env, b"AUDIT", &credit_id, &auditor, records.len()),
        auditor: auditor.clone(),
        audit_date: env.ledger().timestamp(),
        verified_amount: credit.carbon_tonnes,
        issues_found: issues,
        approved,
        audit_notes: Bytes::from_slice(env, b"Audit complete"),
    };

    // Store verification record
    records.push_back(record.clone());
    env.storage()
        .persistent()
        .set(&CarbonCreditKey::VerificationRecords(credit_id.clone()), &records);

    emit_carbon_credit_event(
        env,
        CarbonCreditEvent::CreditAudited(credit_id, auditor, measured_energy, approved),
    );

    record
}

/// Verify offset authenticity
pub fn verify_offset_authenticity(
    env: &Env,
    credit_id: BytesN<32>,
    verifier: Address,
) -> bool {
    verifier.require_auth();

    let credit: CarbonCredit = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::CarbonCredit(credit_id))
        .unwrap_or_else(|| panic!("Credit not found"));

    // Verify offset has valid verifier
    if credit.offset.verification_body != verifier {
        return false;
    }

    // Verify offset has not expired
    if credit.offset.expiration_date < env.ledger().timestamp() {
        return false;
    }

    true
}

/// Register credit in registry
pub fn register_credit(
    env: &Env,
    caller: Address,
    credit_id: BytesN<32>,
    registry_id: Bytes,
) -> bool {
    caller.require_auth();
    if RbacManager::require_role_min(env, &caller, Role::Submitter).is_err() {
        return false;
    }

    let mut credit: CarbonCredit = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::CarbonCredit(credit_id.clone()))
        .unwrap_or_else(|| panic!("Credit not found"));

    // Update registry registration date
    credit.registry.registry_id = registry_id.clone();
    credit.registry.issuance_date = env.ledger().timestamp();
    credit.version += 1;

    env.storage()
        .persistent()
        .set(&CarbonCreditKey::CarbonCredit(credit_id.clone()), &credit);

    env.storage()
        .persistent()
        .set(&CarbonCreditKey::RegistryEntry(registry_id.clone()), &credit);

    emit_carbon_credit_event(
        env,
        CarbonCreditEvent::CreditRegistered(credit_id, caller, registry_id),
    );

    true
}

/// Update registry entry
pub fn update_registry(
    env: &Env,
    registry_id: Bytes,
    verified_by: Address,
) -> bool {
    verified_by.require_auth();
    if RbacManager::require_role_min(env, &verified_by, Role::Admin).is_err() {
        return false;
    }

    if let Some(mut credit) = env
        .storage()
        .persistent()
        .get::<_, CarbonCredit>(&CarbonCreditKey::RegistryEntry(registry_id.clone()))
    {
        credit.registry.verified_by = verified_by.clone();
        env.storage()
            .persistent()
            .set(&CarbonCreditKey::RegistryEntry(registry_id.clone()), &credit);

        emit_carbon_credit_event(
            env,
            CarbonCreditEvent::RegistryUpdated(registry_id, verified_by),
        );

        true
    } else {
        false
    }
}

/// Link credit to compliance standard
pub fn link_to_standard(
    env: &Env,
    caller: Address,
    credit_id: BytesN<32>,
    standard: ComplianceStandard,
) -> bool {
    caller.require_auth();
    if RbacManager::require_role_min(env, &caller, Role::Submitter).is_err() {
        return false;
    }

    let mut credit: CarbonCredit = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::CarbonCredit(credit_id.clone()))
        .unwrap_or_else(|| panic!("Credit not found"));

    let standard_code = standard.clone() as u32;
    credit.registry.compliance_standard = standard;
    credit.version += 1;

    env.storage()
        .persistent()
        .set(&CarbonCreditKey::CarbonCredit(credit_id.clone()), &credit);

    emit_carbon_credit_event(
        env,
        CarbonCreditEvent::CreditLinkedToStandard(credit_id, caller, standard_code),
    );

    true
}

/// Verify registry compliance
pub fn verify_registry_compliance(
    env: &Env,
    credit_id: BytesN<32>,
) -> bool {
    let credit: CarbonCredit = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::CarbonCredit(credit_id))
        .unwrap_or_else(|| panic!("Credit not found"));

    // Check if credit is registered
    if credit.registry.registry_id.is_empty() {
        return false;
    }

    // Check if verification records exist
    if credit.verification_records.is_empty() {
        return false;
    }

    true
}

/// Calculate carbon reduction
pub fn calculate_carbon_reduction(
    env: &Env,
    credit_id: BytesN<32>,
) -> u32 {
    let credit: CarbonCredit = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::CarbonCredit(credit_id))
        .unwrap_or_else(|| panic!("Credit not found"));

    credit.carbon_tonnes
}

/// Generate offset report
pub fn generate_offset_report(
    env: &Env,
    start_date: u64,
    end_date: u64,
) -> CarbonReductionReport {
    CarbonReductionReport {
        reporting_period: (start_date, end_date),
        total_verified_reduction: 0,
        renewable_energy_mwh: 0,
        offsets_purchased: 0,
        credits_retired: env
            .storage()
            .persistent()
            .get(&CarbonCreditKey::RetiredCount)
            .unwrap_or(0),
        facilities_audited: 0,
        compliance_rate: 95,
    }
}

/// Get portfolio status
pub fn get_portfolio_status(
    env: &Env,
    holder: Address,
) -> PortfolioStatus {
    let credits: Vec<BytesN<32>> = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::HolderCredits(holder.clone()))
        .unwrap_or_else(|| Vec::new(env));

    let mut active = 0;
    let mut retired = 0;
    let mut total_co2e_retired = 0;

    for credit_id in credits.iter() {
        if let Some(credit) = env
            .storage()
            .persistent()
            .get::<_, CarbonCredit>(&CarbonCreditKey::CarbonCredit(credit_id))
        {
            if credit.status == CreditStatus::Retired {
                retired += 1;
                total_co2e_retired += credit.carbon_tonnes;
            } else if credit.status == CreditStatus::Active {
                active += 1;
            }
        }
    }

    PortfolioStatus {
        holder,
        total_credits: credits.len(),
        active_credits: active,
        retired_credits: retired,
        total_co2e_retired,
        portfolio_value_usd: active * 15, // Assuming $15 per credit
        last_updated: env.ledger().timestamp(),
    }
}

/// Validate sustainability claim
pub fn validate_claim(
    _env: &Env,
    claim: SustainabilityClaim,
) -> bool {
    // Check claim has description
    if claim.claim_description.is_empty() {
        return false;
    }

    // Check claim has supporting evidence
    if claim.supporting_evidence.is_empty() {
        return false;
    }

    // Check claimed reduction is reasonable (< 10 million tonnes)
    if claim.claimed_reduction > 10_000_000 {
        return false;
    }

    true
}

/// Verify standard compliance
pub fn verify_standard_compliance(
    env: &Env,
    credit_id: BytesN<32>,
    standard: ComplianceStandard,
) -> bool {
    let credit: CarbonCredit = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::CarbonCredit(credit_id))
        .unwrap_or_else(|| panic!("Credit not found"));

    credit.registry.compliance_standard == standard
}

/// Check data integrity
pub fn check_data_integrity(
    env: &Env,
    credit_id: BytesN<32>,
) -> bool {
    let credit: CarbonCredit = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::CarbonCredit(credit_id))
        .unwrap_or_else(|| panic!("Credit not found"));

    // Verify carbon amount is reasonable
    if credit.carbon_tonnes == 0 || credit.carbon_tonnes > 1_000_000 {
        return false;
    }

    // Verify renewable energy is reasonable
    if credit.renewable_source.energy_generated_mwh > 1_000_000 {
        return false;
    }

    // Verify offset expiration is in future
    if credit.offset.expiration_date < env.ledger().timestamp() {
        return false;
    }

    true
}

/// Get total retired CO2e
pub fn get_total_retired_co2e(env: &Env) -> u32 {
    env.storage()
        .persistent()
        .get(&CarbonCreditKey::TotalCo2eRetired)
        .unwrap_or(0)
}

/// Get credit status
pub fn get_credit_status(env: &Env, credit_id: BytesN<32>) -> CreditStatus {
    let credit: CarbonCredit = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::CarbonCredit(credit_id))
        .unwrap_or_else(|| panic!("Credit not found"));

    credit.status
}

/// Get credit details
pub fn get_credit_details(env: &Env, credit_id: BytesN<32>) -> CarbonCredit {
    env.storage()
        .persistent()
        .get(&CarbonCreditKey::CarbonCredit(credit_id))
        .unwrap_or_else(|| panic!("Credit not found"))
}

/// Get issuer credits
pub fn get_issuer_credits(env: &Env, issuer: Address) -> Vec<BytesN<32>> {
    env.storage()
        .persistent()
        .get(&CarbonCreditKey::IssuerCredits(issuer))
        .unwrap_or_else(|| Vec::new(env))
}

/// Get holder credits
pub fn get_holder_credits(env: &Env, holder: Address) -> Vec<BytesN<32>> {
    env.storage()
        .persistent()
        .get(&CarbonCreditKey::HolderCredits(holder))
        .unwrap_or_else(|| Vec::new(env))
}

/// Get total credits issued
pub fn get_total_credits_issued(env: &Env) -> u32 {
    env.storage()
        .persistent()
        .get(&CarbonCreditKey::CreditCounter)
        .unwrap_or(0)
}

