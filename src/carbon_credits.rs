#![allow(dead_code)]

use crate::rbac::{RbacManager, Role};
use soroban_sdk::{contracterror, contracttype, Address, Bytes, BytesN, Env, String, Symbol, Vec};

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
    /// Page limit exceeds maximum allowed (100)
    PageLimitExceeded = 3013,
    /// Invalid cursor value
    InvalidCursor = 3014,
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
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CarbonCreditEvent {
    CreditIssued {
        credit_id: BytesN<32>,
        issuer: Address,
        carbon_tonnes: u32,
    },
    CreditVerified {
        credit_id: BytesN<32>,
        verifier: Address,
        energy_mwh: u32,
    },
    CreditTokenized {
        credit_id: BytesN<32>,
        token_owner: Address,
        total_tokens: u128,
    },
    CreditRetired {
        credit_id: BytesN<32>,
        caller: Address,
        carbon_tonnes: u32,
    },
    CreditTransferred {
        credit_id: BytesN<32>,
        from: Address,
        to: Address,
    },
    CreditAudited {
        credit_id: BytesN<32>,
        auditor: Address,
        measured_energy: u32,
        approved: bool,
    },
    CreditRegistered {
        credit_id: BytesN<32>,
        caller: Address,
        registry_id: Bytes,
    },
    RegistryUpdated {
        registry_id: Bytes,
        verified_by: Address,
    },
    CreditLinkedToStandard {
        credit_id: BytesN<32>,
        caller: Address,
        standard: u32,
    },
}

/// Emit a structured, tamper-evident audit event for a carbon credit mutation.
fn emit_carbon_credit_event(env: &Env, event: CarbonCreditEvent) {
    env.events().publish(
        (Symbol::new(&env, "carbon"), Symbol::new(&env, "credit")),
        event,
    );
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
// Pagination Types (issue #882)
// ─────────────────────────────────────────────────────────────────────────────

/// Maximum number of items that can be returned in a single page.
pub const MAX_PAGE_LIMIT: u32 = 100;

/// Opaque cursor for paginated queries.
///
/// Encodes a zero-based index into the ordered credit list.  A value of `0`
/// means "start from the beginning".  Callers should treat this as an opaque
/// token obtained from `CreditPage.next_cursor`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageCursor {
    /// Zero-based index into the ordered result set.
    pub index: u32,
}

/// Requested page size.  Capped at [`MAX_PAGE_LIMIT`] (100).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageLimit {
    /// Maximum number of items to return (1–100).
    pub limit: u32,
}

/// Filter predicates for credit queries.
///
/// All fields are optional; `None` means "no filter on this dimension".
/// When multiple filters are supplied, they are AND-combined.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreditQueryFilter {
    /// Inclusive lower bound on `creation_date`.
    pub created_after: Option<u64>,
    /// Inclusive upper bound on `creation_date`.
    pub created_before: Option<u64>,
    /// Filter by credit status.
    pub status: Option<CreditStatus>,
    /// Filter by issuer/submitter address.
    pub submitter: Option<Address>,
}

/// A page of paginated credit results.
///
/// Contains the credit IDs on the current page, the total count of matching
/// items, and an opaque cursor to fetch the next page.  When `next_cursor`
/// is `None`, there are no more results.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreditPage {
    /// Credit IDs on this page.
    pub items: Vec<BytesN<32>>,
    /// Total number of credits matching the filter.
    pub total: u32,
    /// Cursor to use for the next page, or `None` if this is the last page.
    pub next_cursor: Option<PageCursor>,
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
    /// Sequential list of all credit IDs for deterministic pagination ordering
    AllCreditIds,
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
        return BytesN::from_array(&env, &[0u8; 32]);
    }

    let mut preimage = Bytes::new(env);
    preimage.append(&Bytes::from_array(env, b"CREDIT"));
    // Append carbon_tonnes as 4 bytes LE
    preimage.append(&Bytes::from_array(env, &carbon_tonnes.to_le_bytes()));
    // Append issuer string representation
    preimage.append(&issuer.to_string().to_bytes());
    // Append timestamp as 8 bytes LE
    preimage.append(&Bytes::from_array(env, &env.ledger().timestamp().to_le_bytes()));
    let credit_id: BytesN<32> = env.crypto().sha256(&preimage).into();

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
            token_id: BytesN::from_array(&env, &[0u8; 32]),
            total_tokens: 0,
            tokens_retired: 0,
            token_owner: Address::from_string(&String::from_str(&env, crate::NULL_ACCOUNT)),
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
        .set(&CarbonCreditKey::IssuerCredits(issuer), &issuer_credits);

    // Add to global ordered list for deterministic pagination (issue #882)
    let mut all_ids: Vec<BytesN<32>> = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::AllCreditIds)
        .unwrap_or_else(|| Vec::new(env));
    all_ids.push_back(credit_id.clone());
    env.storage()
        .persistent()
        .set(&CarbonCreditKey::AllCreditIds, &all_ids);

    // Increment counter
    let count: u32 = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::CreditCounter)
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&CarbonCreditKey::CreditCounter, &(count + 1));

    emit_carbon_credit_event(
        env,
        CarbonCreditEvent::CreditIssued {
            credit_id: credit_id.clone(),
            issuer: issuer.clone(),
            carbon_tonnes,
        },
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

    // Verify energy generation matches
    if credit.renewable_source.energy_generated_mwh != energy_mwh {
        return false;
    }

    // Create verification record
    let verification = VerificationRecord {
        verification_id: env.crypto().sha256(&Bytes::from_array(env, b"VERIFY")).into(),
        auditor: verifier,
        audit_date: env.ledger().timestamp(),
        verified_amount: credit.carbon_tonnes,
        issues_found: Vec::new(env),
        approved: true,
        audit_notes: Bytes::from_array(env, b"Renewable energy verified"),
    };

    credit.verification_records.push_back(verification);
    credit.status = CreditStatus::Active;

    env.storage()
        .persistent()
        .set(&CarbonCreditKey::CarbonCredit(credit_id), &credit);

    emit_carbon_credit_event(
        env,
        CarbonCreditEvent::CreditVerified {
            credit_id,
            verifier,
            energy_mwh,
        },
    );

    true
}

/// Calculate carbon offset based on renewable energy
pub fn calculate_offset(_env: &Env, energy_mwh: u32) -> u32 {
    // Typical calculation: 1 MWh ≈ 0.5 tonnes CO2e offset
    (energy_mwh / 2) as u32
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
        return BytesN::from_array(&env, &[0u8; 32]);
    }

    let mut credit: CarbonCredit = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::CarbonCredit(credit_id.clone()))
        .unwrap_or_else(|| panic!("Credit not found"));

    let mut token_preimage = Bytes::new(env);
    token_preimage.append(&Bytes::from_array(env, b"TOKEN"));
    token_preimage.append(&token_owner.to_string().to_bytes());
    token_preimage.append(&Bytes::from_array(env, &env.ledger().timestamp().to_le_bytes()));
    let token_id: BytesN<32> = env.crypto().sha256(&token_preimage).into();

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
        CarbonCreditEvent::CreditTokenized {
            credit_id,
            token_owner,
            total_tokens: tokens_to_issue,
        },
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
        CarbonCreditEvent::CreditRetired {
            credit_id,
            caller,
            carbon_tonnes,
        },
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
    RbacManager::require_role_min(env, &from, Role::Submitter)?;

    let mut credit: CarbonCredit = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::CarbonCredit(credit_id.clone()))
        .unwrap_or_else(|| panic!("Credit not found"));

    if credit.status == CreditStatus::Retired {
        return false; // Can't transfer retired credits
    }

    // Update tokenization owner if applicable
    if credit.tokenization.token_id != BytesN::from_array(&env, &[0u8; 32]) {
        credit.tokenization.token_owner = to.clone();
    }

    env.storage()
        .persistent()
        .set(&CarbonCreditKey::CarbonCredit(credit_id.clone()), &credit);

    // Update holder index
    let mut to_credits: Vec<BytesN<32>> = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::HolderCredits(to.clone()))
        .unwrap_or_else(|| Vec::new(env));
    to_credits.push_back(credit_id);
    env.storage()
        .persistent()
        .set(&CarbonCreditKey::HolderCredits(to), &to_credits);

    emit_carbon_credit_event(
        env,
        CarbonCreditEvent::CreditTransferred {
            credit_id,
            from,
            to,
        },
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
            verification_id: BytesN::from_array(&env, &[0u8; 32]),
            auditor,
            audit_date: env.ledger().timestamp(),
            verified_amount: 0,
            issues_found: vec![&env, Bytes::from_slice(&env, b"insufficient role")],
            approved: false,
            audit_notes: Bytes::from_slice(&env, b"Unauthorized auditor"),
        };
    }

    let credit: CarbonCredit = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::CarbonCredit(credit_id.clone()))
        .unwrap_or_else(|| panic!("Credit not found"));

    let mut issues = Vec::new(env);
    let mut approved = true;

    // Check if measured energy matches recorded
    if (credit.renewable_source.energy_generated_mwh as i32
        - measured_energy as i32)
        .abs() > 100
    {
        issues.push_back(Bytes::from_array(env, b"Energy mismatch"));
        approved = false;
    }

    let record = VerificationRecord {
        verification_id: env.crypto().sha256(&Bytes::from_array(env, b"AUDIT")).into(),
        auditor,
        audit_date: env.ledger().timestamp(),
        verified_amount: credit.carbon_tonnes,
        issues_found: issues,
        approved,
        audit_notes: Bytes::from_array(env, b"Audit complete"),
    };

    // Store verification record
    let mut records: Vec<VerificationRecord> = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::VerificationRecords(credit_id.clone()))
        .unwrap_or_else(|| Vec::new(env));
    records.push_back(record.clone());
    env.storage()
        .persistent()
        .set(&CarbonCreditKey::VerificationRecords(credit_id), &records);

    emit_carbon_credit_event(
        env,
        CarbonCreditEvent::CreditAudited {
            credit_id,
            auditor,
            measured_energy,
            approved,
        },
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

    env.storage()
        .persistent()
        .set(&CarbonCreditKey::CarbonCredit(credit_id), &credit);

    env.storage()
        .persistent()
        .set(&CarbonCreditKey::RegistryEntry(registry_id), &credit);

    emit_carbon_credit_event(
        env,
        CarbonCreditEvent::CreditRegistered {
            credit_id,
            caller,
            registry_id,
        },
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
            .set(&CarbonCreditKey::RegistryEntry(registry_id), &credit);

        emit_carbon_credit_event(
            env,
            CarbonCreditEvent::RegistryUpdated {
                registry_id,
                verified_by,
            },
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

    credit.registry.compliance_standard = standard;

    env.storage()
        .persistent()
        .set(&CarbonCreditKey::CarbonCredit(credit_id), &credit);

    emit_carbon_credit_event(
        env,
        CarbonCreditEvent::CreditLinkedToStandard {
            credit_id,
            caller,
            standard: standard as u32,
        },
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
    let report = CarbonReductionReport {
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
    };

    report
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
        total_credits: credits.len() as u32,
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

// ─────────────────────────────────────────────────────────────────────────────
// Paginated Query Functions (issue #882)
// ─────────────────────────────────────────────────────────────────────────────

/// Validate and normalize a [`PageLimit`].
///
/// Returns a clamped limit in the range `[1, MAX_PAGE_LIMIT]`.
fn validate_page_limit(limit: &PageLimit) -> u32 {
    if limit.limit == 0 {
        1
    } else if limit.limit > MAX_PAGE_LIMIT {
        MAX_PAGE_LIMIT
    } else {
        limit.limit
    }
}

/// Check whether a credit matches the given filter predicates.
///
/// All supplied predicates must match (AND semantics).  `None` fields are
/// treated as "no constraint on this dimension".
fn credit_matches_filter(
    env: &Env,
    credit_id: &BytesN<32>,
    filter: &CreditQueryFilter,
) -> bool {
    let credit: CarbonCredit = match env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::CarbonCredit(credit_id.clone()))
    {
        Some(c) => c,
        None => return false,
    };

    // Timestamp range filter
    if let Some(after) = filter.created_after {
        if credit.creation_date < after {
            return false;
        }
    }
    if let Some(before) = filter.created_before {
        if credit.creation_date > before {
            return false;
        }
    }

    // Status filter
    if let Some(ref status) = filter.status {
        if &credit.status != status {
            return false;
        }
    }

    // Submitter (issuer) filter
    if let Some(ref submitter) = filter.submitter {
        if &credit.issuer != submitter {
            return false;
        }
    }

    true
}

/// Paginated query for all carbon credit IDs with optional filtering.
///
/// Credits are returned in deterministic insertion order (oldest first),
/// ensuring stable ordering across ledger upgrades and archival restores.
///
/// # Arguments
/// * `env` — Soroban environment.
/// * `cursor` — Starting position; pass `PageCursor { index: 0 }` for the first page.
/// * `limit` — Maximum items per page (clamped to `[1, 100]`).
/// * `filter` — Optional filter predicates.  All fields `None` = no filtering.
///
/// # Returns
/// A [`CreditPage`] containing matching credit IDs, total count, and an
/// optional `next_cursor` for the subsequent page.
///
/// # CPU/Memory Safety
/// Page size is hard-capped at 100 to stay within Soroban single-invocation
/// CPU and memory limits.  The scan is bounded: at most `limit` items are
/// materialised per call.
pub fn query_credits_paginated(
    env: &Env,
    cursor: PageCursor,
    limit: PageLimit,
    filter: CreditQueryFilter,
) -> CreditPage {
    let effective_limit = validate_page_limit(&limit);

    let all_ids: Vec<BytesN<32>> = env
        .storage()
        .persistent()
        .get(&CarbonCreditKey::AllCreditIds)
        .unwrap_or_else(|| Vec::new(env));

    let total_count = all_ids.len();
    let start = cursor.index;

    // If cursor is past the end, return an empty page
    if start >= total_count {
        return CreditPage {
            items: Vec::new(env),
            total: total_count,
            next_cursor: None,
        };
    }

    let mut items: Vec<BytesN<32>> = Vec::new(env);
    let mut scanned = 0u32;
    let max_scan = total_count; // Safety: never scan beyond the stored list

    let mut idx = start;
    while idx < max_scan && items.len() < effective_limit {
        let credit_id = all_ids.get(idx).unwrap();

        if credit_matches_filter(env, &credit_id, &filter) {
            items.push_back(credit_id);
        }

        idx += 1;
        scanned += 1;

        // Guard against unbounded scanning (CPU budget protection)
        if scanned >= MAX_PAGE_LIMIT * 10 {
            break;
        }
    }

    // Determine next cursor: if we consumed `effective_limit` items and there
    // are more entries to scan, the next cursor is the current scan position.
    let next_cursor = if idx < total_count {
        Some(PageCursor { index: idx })
    } else {
        None
    };

    CreditPage {
        items,
        total: total_count,
        next_cursor,
    }
}

/// Paginated query for credit IDs filtered by status only.
///
/// Convenience wrapper around [`query_credits_paginated`] for status-only
/// filtering (e.g., "list all retired credits").
pub fn query_credits_by_status(
    env: &Env,
    cursor: PageCursor,
    limit: PageLimit,
    status: CreditStatus,
) -> CreditPage {
    query_credits_paginated(
        env,
        cursor,
        limit,
        CreditQueryFilter {
            created_after: None,
            created_before: None,
            status: Some(status),
            submitter: None,
        },
    )
}

/// Paginated query for credit IDs filtered by submitter (issuer) address.
pub fn query_credits_by_submitter(
    env: &Env,
    cursor: PageCursor,
    limit: PageLimit,
    submitter: Address,
) -> CreditPage {
    query_credits_paginated(
        env,
        cursor,
        limit,
        CreditQueryFilter {
            created_after: None,
            created_before: None,
            status: None,
            submitter: Some(submitter),
        },
    )
}

/// Paginated query for credit IDs filtered by creation timestamp range.
pub fn query_credits_by_time_range(
    env: &Env,
    cursor: PageCursor,
    limit: PageLimit,
    created_after: u64,
    created_before: u64,
) -> CreditPage {
    query_credits_paginated(
        env,
        cursor,
        limit,
        CreditQueryFilter {
            created_after: Some(created_after),
            created_before: Some(created_before),
            status: None,
            submitter: None,
        },
    )
}

/// Fetch full credit details for a page of credit IDs.
///
/// Given a [`CreditPage`] of IDs, materialises the full [`CarbonCredit`]
/// structs.  This keeps the paginated query itself lightweight (returning
/// only IDs) and defers the heavier struct deserialization to this call.
pub fn get_credits_from_page(
    env: &Env,
    page: &CreditPage,
) -> Vec<CarbonCredit> {
    let mut credits = Vec::new(env);
    for i in 0..page.items.len() {
        let credit_id = page.items.get(i).unwrap();
        if let Some(credit) = env
            .storage()
            .persistent()
            .get::<_, CarbonCredit>(&CarbonCreditKey::CarbonCredit(credit_id))
        {
            credits.push_back(credit);
        }
    }
    credits
}
