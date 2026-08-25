/// # Anti-Corruption Compliance Module
///
/// Comprehensive anti-corruption and anti-bribery compliance framework
/// implementing FCPA (Foreign Corrupt Practices Act) and UK Bribery Act
/// requirements with risk assessment, policies, training tracking, due
/// diligence, continuous monitoring, incident reporting, third-party risk
/// management, and blockchain-anchored whistleblower mechanisms.
///
/// ## Regulatory Framework
/// - **FCPA** — U.S. Foreign Corrupt Practices Act (anti-bribery, books & records, accounting controls)
/// - **UK Bribery Act** — Bribery Act 2010 (corporate offense, commercial bribes)
/// - **SOX** — Sarbanes-Oxley Act (internal controls, audit trails)
/// - **COSO** — Committee of Sponsoring Organizations (fraud risk assessment)

use soroban_sdk::{
    bytes, contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Bytes,
    BytesN, Env, Symbol, Vec,
};

// ── Error Codes ──────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum AntiCorruptionError {
    /// Policy not found or inactive
    PolicyNotFound = 2000,
    /// Risk assessment shows high corruption risk
    HighCorruptionRisk = 2001,
    /// Employee training not completed
    TrainingNotCompleted = 2002,
    /// Third-party risk assessment incomplete
    ThirdPartyRiskNotAssessed = 2003,
    /// Prohibited transaction or payment
    ProhibitedTransaction = 2004,
    /// Gift/entertainment exceeds policy limits
    GiftLimitExceeded = 2005,
    /// Government official interaction undisclosed
    GovOfficialUndisclosed = 2006,
    /// High-risk jurisdiction transaction
    HighRiskJurisdiction = 2007,
    /// Sanctions list match detected
    SanctionsListMatch = 2008,
    /// Whistleblower report sealed or contested
    WhistleblowerReportSealed = 2009,
    /// Due diligence verification failed
    DueDiligenceFailed = 2010,
    /// Beneficial owner not disclosed
    BeneficialOwnerUndisclosed = 2011,
    /// Political exposure (PEP) detected
    PoliticalExposureDetected = 2012,
    /// Compliance violation detected
    ComplianceViolation = 2013,
    /// Investigation still ongoing
    InvestigationOngoing = 2014,
    /// Unauthorized whistleblower access
    UnauthorizedWhistleblowerAccess = 2015,
}

// ── Data Types ───────────────────────────────────────────────────────────

/// Compliance policy type
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum PolicyType {
    /// Anti-bribery and corruption policy
    AntiBriberyCorruption = 1,
    /// Gifts and entertainment policy
    GiftsEntertainment = 2,
    /// Conflict of interest policy
    ConflictOfInterest = 3,
    /// Insider trading policy
    InsiderTrading = 4,
    /// Government relations policy
    GovernmentRelations = 5,
    /// Sanctions and export controls policy
    SanctionsExportControls = 6,
}

/// Risk assessment classification
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum RiskLevel {
    /// Minimal corruption risk
    Low = 1,
    /// Moderate risk requiring monitoring
    Medium = 2,
    /// Significant risk requiring mitigation
    High = 3,
    /// Critical risk requiring escalation
    Critical = 4,
}

/// Training status
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum TrainingStatus {
    /// Training not started
    NotStarted = 0,
    /// Training in progress
    InProgress = 1,
    /// Training completed
    Completed = 2,
    /// Training overdue
    Overdue = 3,
}

/// Transaction type for monitoring
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum TransactionType {
    /// Payment to government official
    GovernmentPayment = 1,
    /// Gift or entertainment
    GiftEntertainment = 2,
    /// Third-party intermediary payment
    ThirdPartyPayment = 3,
    /// Charitable donation
    CharitableDonation = 4,
    /// Travel and accommodation
    TravelAccommodation = 5,
    /// Facilitation payment (small payment for expediting services)
    FacilitationPayment = 6,
}

/// Whistleblower report status
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum WhistleblowerStatus {
    /// Report submitted, under review
    Submitted = 1,
    /// Report acknowledged, investigation initiated
    Acknowledged = 2,
    /// Investigation underway
    InProgress = 3,
    /// Investigation concluded, findings documented
    Concluded = 4,
    /// Corrective actions implemented
    Resolved = 5,
}

/// Compliance policy
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompliancePolicy {
    /// Policy ID
    pub id: BytesN<32>,
    /// Policy type
    pub policy_type: u32,
    /// Policy title
    pub title: Bytes,
    /// Policy description
    pub description: Bytes,
    /// Effective date
    pub effective_date: u64,
    /// Last updated
    pub last_updated: u64,
    /// Policy version
    pub version: u32,
    /// Is active
    pub active: bool,
    /// Policy content hash
    pub content_hash: BytesN<32>,
}

/// Anti-corruption risk assessment
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiskAssessment {
    /// Assessment ID
    pub id: BytesN<32>,
    /// Subject being assessed (entity/individual)
    pub subject: Address,
    /// Risk level
    pub risk_level: u32,
    /// Assessment timestamp
    pub assessed_at: u64,
    /// Assessed by
    pub assessed_by: Address,
    /// Risk factors identified
    pub risk_factors: Vec<Bytes>,
    /// Mitigation measures
    pub mitigations: Vec<Bytes>,
    /// Next review date
    pub next_review_date: u64,
    /// Assessment hash
    pub assessment_hash: BytesN<32>,
}

/// Employee training record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrainingRecord {
    /// Training ID
    pub id: BytesN<32>,
    /// Employee address
    pub employee: Address,
    /// Training type
    pub training_type: u32,
    /// Training status
    pub status: u32,
    /// Started date
    pub started_at: u64,
    /// Completed date (0 if not completed)
    pub completed_at: u64,
    /// Due date
    pub due_date: u64,
    /// Score/certification
    pub score: u32,
    /// Training hash
    pub training_hash: BytesN<32>,
}

/// Third-party risk profile
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThirdPartyRisk {
    /// Risk profile ID
    pub id: BytesN<32>,
    /// Third party address
    pub third_party: Address,
    /// Third party name
    pub name: Bytes,
    /// Country of operation
    pub country: Bytes,
    /// Industry sector
    pub sector: Bytes,
    /// Risk level
    pub risk_level: u32,
    /// PEP (Politically Exposed Person) status
    pub is_pep: bool,
    /// Sanctions list match detected
    pub sanctions_match: bool,
    /// Due diligence completed
    pub due_diligence_completed: bool,
    /// Due diligence date
    pub due_diligence_date: u64,
    /// Beneficial owners disclosed
    pub beneficial_owners_disclosed: bool,
    /// Last review timestamp
    pub last_review_at: u64,
    /// Risk profile hash
    pub risk_hash: BytesN<32>,
}

/// Monitored transaction
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MonitoredTransaction {
    /// Transaction ID
    pub id: BytesN<32>,
    /// From party
    pub from: Address,
    /// To party
    pub to: Address,
    /// Transaction type
    pub tx_type: u32,
    /// Amount
    pub amount: u64,
    /// Currency (e.g., "USD", "EUR")
    pub currency: Bytes,
    /// Description
    pub description: Bytes,
    /// Transaction date
    pub tx_date: u64,
    /// Risk flags (if any)
    pub risk_flags: Vec<Bytes>,
    /// Approved/rejected
    pub status: u32, // 0=pending, 1=approved, 2=rejected
    /// Approval date
    pub approval_date: u64,
    /// Approved by
    pub approved_by: Address,
    /// Transaction hash
    pub tx_hash: BytesN<32>,
}

/// Whistleblower report (encrypted/confidential)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WhistleblowerReport {
    /// Report ID
    pub id: BytesN<32>,
    /// Reporter (can be confidential)
    pub reporter: Address,
    /// Report title
    pub title: Bytes,
    /// Report description (encrypted)
    pub description_encrypted: Bytes,
    /// Reported at
    pub reported_at: u64,
    /// Status
    pub status: u32,
    /// Assigned investigator
    pub investigator: Address,
    /// Investigation findings (when concluded)
    pub findings_encrypted: Bytes,
    /// Corrective actions (when resolved)
    pub corrective_actions: Bytes,
    /// Reporter contact (encrypted)
    pub reporter_contact_encrypted: Bytes,
    /// Confidentiality level: 0=public, 1=internal, 2=restricted, 3=secret
    pub confidentiality_level: u32,
    /// Report hash
    pub report_hash: BytesN<32>,
}

/// Compliance incident
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComplianceIncident {
    /// Incident ID
    pub id: BytesN<32>,
    /// Incident type
    pub incident_type: Bytes,
    /// Description
    pub description: Bytes,
    /// Detected date
    pub detected_at: u64,
    /// Reported by
    pub reported_by: Address,
    /// Severity: 1=low, 2=medium, 3=high, 4=critical
    pub severity: u32,
    /// Status: 0=reported, 1=investigating, 2=resolved
    pub status: u32,
    /// Root cause analysis
    pub root_cause: Bytes,
    /// Corrective actions
    pub corrective_actions: Bytes,
    /// Due date for remediation
    pub remediation_due_date: u64,
    /// Incident hash
    pub incident_hash: BytesN<32>,
}

/// High-risk jurisdiction list entry
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HighRiskJurisdiction {
    /// Jurisdiction code (ISO 3166-1 alpha-2)
    pub country_code: Bytes,
    /// Country name
    pub country_name: Bytes,
    /// Risk factors
    pub risk_factors: Vec<Bytes>,
    /// Restrictions (0=advisory, 1=screening, 2=prohibition)
    pub restriction_level: u32,
    /// Last updated
    pub updated_at: u64,
}

// ── Data Keys ────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum AntiCorruptionKey {
    /// Contract owner
    Owner,
    /// Compliance officer
    ComplianceOfficer,
    /// Policies by ID
    Policy(BytesN<32>),
    /// Risk assessments by ID
    RiskAssessment(BytesN<32>),
    /// Risk assessments by subject
    RiskAssessmentsBySubject(Address),
    /// Training records by ID
    TrainingRecord(BytesN<32>),
    /// Training records by employee
    TrainingRecordsByEmployee(Address),
    /// Third-party risk profiles
    ThirdPartyRisk(BytesN<32>),
    /// Third-party by address
    ThirdPartyByAddress(Address),
    /// Monitored transactions by ID
    MonitoredTransaction(BytesN<32>),
    /// Transactions by initiator
    TransactionsByInitiator(Address),
    /// Whistleblower reports by ID
    WhistleblowerReport(BytesN<32>),
    /// Whistleblower reports by reporter (encrypted)
    WhistleblowerByReporter(Address),
    /// Compliance incidents by ID
    ComplianceIncident(BytesN<32>),
    /// High-risk jurisdictions
    HighRiskJurisdiction(Bytes),
    /// Sanctions list match registry
    SanctionsMatch(Address),
    /// PEP (Politically Exposed Person) registry
    PEPRegistry(Address),
    /// Total policies
    PolicyCount,
    /// Total assessments
    AssessmentCount,
    /// Total training records
    TrainingCount,
    /// Total third-party profiles
    ThirdPartyCount,
    /// Total transactions
    TransactionCount,
    /// Total whistleblower reports
    WhistleblowerCount,
    /// Total compliance incidents
    IncidentCount,
    /// Compliance violations counter
    ViolationCount,
}

// ── Contract Implementation ──────────────────────────────────────────────

#[contract]
pub struct AntiCorruption;

#[contractimpl]
impl AntiCorruption {
    /// Initialize the anti-corruption compliance module
    pub fn initialize(env: Env, owner: Address, compliance_officer: Address) {
        owner.require_auth();
        env.storage()
            .instance()
            .set(&AntiCorruptionKey::Owner, &owner);
        env.storage()
            .instance()
            .set(&AntiCorruptionKey::ComplianceOfficer, &compliance_officer);
        env.storage()
            .instance()
            .set(&AntiCorruptionKey::PolicyCount, &0u32);
        env.storage()
            .instance()
            .set(&AntiCorruptionKey::AssessmentCount, &0u32);
        env.storage()
            .instance()
            .set(&AntiCorruptionKey::TrainingCount, &0u32);
        env.storage()
            .instance()
            .set(&AntiCorruptionKey::ThirdPartyCount, &0u32);
        env.storage()
            .instance()
            .set(&AntiCorruptionKey::TransactionCount, &0u32);
        env.storage()
            .instance()
            .set(&AntiCorruptionKey::WhistleblowerCount, &0u32);
        env.storage()
            .instance()
            .set(&AntiCorruptionKey::IncidentCount, &0u32);
        env.storage()
            .instance()
            .set(&AntiCorruptionKey::ViolationCount, &0u32);
    }

    // ── Policy Management ────────────────────────────────────────────────

    /// Publish a compliance policy
    pub fn publish_policy(
        env: Env,
        caller: Address,
        policy_type: u32,
        title: Bytes,
        description: Bytes,
        policy_content: Bytes,
    ) -> BytesN<32> {
        caller.require_auth();
        Self::require_compliance_officer(&env, &caller);

        let policy_id = Self::compute_policy_id(&env, &title, policy_content.clone());
        let now = env.ledger().timestamp();
        let content_hash = env.crypto().sha256(&policy_content).into();

        let policy = CompliancePolicy {
            id: policy_id.clone(),
            policy_type,
            title: title.clone(),
            description,
            effective_date: now,
            last_updated: now,
            version: 1,
            active: true,
            content_hash,
        };

        env.storage()
            .instance()
            .set(&AntiCorruptionKey::Policy(policy_id.clone()), &policy);

        let count: u32 = env
            .storage()
            .instance()
            .get(&AntiCorruptionKey::PolicyCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&AntiCorruptionKey::PolicyCount, &(count + 1));

        env.events().publish(
            (Symbol::new(&env, "policy_published"),),
            (policy_id.clone(), policy_type, title),
        );

        policy_id
    }

    /// Get a compliance policy
    pub fn get_policy(env: Env, policy_id: BytesN<32>) -> CompliancePolicy {
        env.storage()
            .instance()
            .get(&AntiCorruptionKey::Policy(policy_id.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, AntiCorruptionError::PolicyNotFound))
    }

    /// Update policy version
    pub fn update_policy(
        env: Env,
        caller: Address,
        policy_id: BytesN<32>,
        new_content: Bytes,
    ) {
        caller.require_auth();
        Self::require_compliance_officer(&env, &caller);

        let mut policy = Self::get_policy(env.clone(), policy_id.clone());
        policy.version += 1;
        policy.last_updated = env.ledger().timestamp();
        policy.content_hash = env.crypto().sha256(&new_content).into();

        env.storage()
            .instance()
            .set(&AntiCorruptionKey::Policy(policy_id.clone()), &policy);

        env.events().publish(
            (Symbol::new(&env, "policy_updated"),),
            (policy_id, policy.version),
        );
    }

    // ── Risk Assessment ──────────────────────────────────────────────────

    /// Perform anti-corruption risk assessment
    pub fn assess_risk(
        env: Env,
        caller: Address,
        subject: Address,
        risk_level: u32,
        risk_factors: Vec<Bytes>,
        mitigations: Vec<Bytes>,
        next_review_days: u32,
    ) -> BytesN<32> {
        caller.require_auth();
        Self::require_compliance_officer(&env, &caller);

        let assessment_id = Self::compute_assessment_id(&env, &subject);
        let now = env.ledger().timestamp();

        let assessment = RiskAssessment {
            id: assessment_id.clone(),
            subject: subject.clone(),
            risk_level,
            assessed_at: now,
            assessed_by: caller.clone(),
            risk_factors: risk_factors.clone(),
            mitigations: mitigations.clone(),
            next_review_date: now + (next_review_days as u64 * 86400),
            assessment_hash: env
                .crypto()
                .sha256(&Self::pack_assessment_data(
                    &env,
                    &subject,
                    risk_level,
                    now,
                ))
                .into(),
        };

        env.storage()
            .instance()
            .set(&AntiCorruptionKey::RiskAssessment(assessment_id.clone()), &assessment);

        env.storage().instance().set(
            &AntiCorruptionKey::RiskAssessmentsBySubject(subject.clone()),
            &assessment_id,
        );

        let count: u32 = env
            .storage()
            .instance()
            .get(&AntiCorruptionKey::AssessmentCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&AntiCorruptionKey::AssessmentCount, &(count + 1));

        env.events().publish(
            (Symbol::new(&env, "risk_assessment_completed"),),
            (assessment_id.clone(), subject, risk_level),
        );

        assessment_id
    }

    /// Get risk assessment for subject
    pub fn get_risk_assessment(env: Env, subject: Address) -> RiskAssessment {
        if let Some(assessment_id) = env
            .storage()
            .instance()
            .get::<_, BytesN<32>>(&AntiCorruptionKey::RiskAssessmentsBySubject(subject.clone()))
        {
            env.storage()
                .instance()
                .get(&AntiCorruptionKey::RiskAssessment(assessment_id))
                .unwrap_or_else(|| panic_with_error!(&env, AntiCorruptionError::HighCorruptionRisk))
        } else {
            panic_with_error!(&env, AntiCorruptionError::HighCorruptionRisk)
        }
    }

    // ── Training Management ──────────────────────────────────────────────

    /// Create training requirement
    pub fn create_training(
        env: Env,
        caller: Address,
        employee: Address,
        training_type: u32,
        due_date: u64,
    ) -> BytesN<32> {
        caller.require_auth();
        Self::require_compliance_officer(&env, &caller);

        let training_id = Self::compute_training_id(&env, &employee, training_type);
        let now = env.ledger().timestamp();

        let training = TrainingRecord {
            id: training_id.clone(),
            employee: employee.clone(),
            training_type,
            status: 0, // NotStarted
            started_at: 0,
            completed_at: 0,
            due_date,
            score: 0,
            training_hash: env
                .crypto()
                .sha256(&Self::pack_training_data(&env, &employee, training_type, now))
                .into(),
        };

        env.storage()
            .instance()
            .set(&AntiCorruptionKey::TrainingRecord(training_id.clone()), &training);

        env.storage().instance().set(
            &AntiCorruptionKey::TrainingRecordsByEmployee(employee.clone()),
            &training_id,
        );

        let count: u32 = env
            .storage()
            .instance()
            .get(&AntiCorruptionKey::TrainingCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&AntiCorruptionKey::TrainingCount, &(count + 1));

        env.events().publish(
            (Symbol::new(&env, "training_assigned"),),
            (training_id.clone(), employee, due_date),
        );

        training_id
    }

    /// Complete training
    pub fn complete_training(env: Env, caller: Address, training_id: BytesN<32>, score: u32) {
        caller.require_auth();

        let mut training: TrainingRecord = env
            .storage()
            .instance()
            .get(&AntiCorruptionKey::TrainingRecord(training_id.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, AntiCorruptionError::TrainingNotCompleted));

        // Verify caller is the employee
        if training.employee != caller && !Self::is_compliance_officer(&env, &caller) {
            panic_with_error!(&env, AntiCorruptionError::TrainingNotCompleted);
        }

        training.status = 2; // Completed
        training.completed_at = env.ledger().timestamp();
        training.score = score;

        env.storage()
            .instance()
            .set(&AntiCorruptionKey::TrainingRecord(training_id.clone()), &training);

        env.events().publish(
            (Symbol::new(&env, "training_completed"),),
            (training_id, caller, score),
        );
    }

    /// Get training record
    pub fn get_training_record(env: Env, training_id: BytesN<32>) -> TrainingRecord {
        env.storage()
            .instance()
            .get(&AntiCorruptionKey::TrainingRecord(training_id))
            .unwrap_or_else(|| panic_with_error!(&env, AntiCorruptionError::TrainingNotCompleted))
    }

    // ── Third-Party Risk Management ──────────────────────────────────────

    /// Assess third-party risk profile
    pub fn assess_third_party(
        env: Env,
        caller: Address,
        third_party: Address,
        name: Bytes,
        country: Bytes,
        sector: Bytes,
        is_pep: bool,
        sanctions_match: bool,
    ) -> BytesN<32> {
        caller.require_auth();
        Self::require_compliance_officer(&env, &caller);

        let risk_id = Self::compute_third_party_id(&env, &third_party, &name);
        let now = env.ledger().timestamp();

        // Calculate risk level based on factors
        let mut risk_level = 1u32; // Low
        if is_pep || sanctions_match {
            risk_level = 4; // Critical
        } else if Self::is_high_risk_jurisdiction(&env, &country) {
            risk_level = 3; // High
        }

        let profile = ThirdPartyRisk {
            id: risk_id.clone(),
            third_party: third_party.clone(),
            name: name.clone(),
            country: country.clone(),
            sector,
            risk_level,
            is_pep,
            sanctions_match,
            due_diligence_completed: false,
            due_diligence_date: 0,
            beneficial_owners_disclosed: false,
            last_review_at: now,
            risk_hash: env
                .crypto()
                .sha256(&Self::pack_third_party_data(
                    &env,
                    &third_party,
                    &name,
                    risk_level,
                ))
                .into(),
        };

        env.storage()
            .instance()
            .set(&AntiCorruptionKey::ThirdPartyRisk(risk_id.clone()), &profile);

        env.storage().instance().set(
            &AntiCorruptionKey::ThirdPartyByAddress(third_party.clone()),
            &risk_id,
        );

        if is_pep {
            env.storage()
                .instance()
                .set(&AntiCorruptionKey::PEPRegistry(third_party.clone()), &true);
        }

        if sanctions_match {
            env.storage()
                .instance()
                .set(&AntiCorruptionKey::SanctionsMatch(third_party.clone()), &true);
        }

        let count: u32 = env
            .storage()
            .instance()
            .get(&AntiCorruptionKey::ThirdPartyCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&AntiCorruptionKey::ThirdPartyCount, &(count + 1));

        env.events().publish(
            (Symbol::new(&env, "third_party_assessed"),),
            (risk_id.clone(), third_party, risk_level),
        );

        risk_id
    }

    /// Complete due diligence on third party
    pub fn complete_due_diligence(
        env: Env,
        caller: Address,
        third_party_id: BytesN<32>,
        beneficial_owners_disclosed: bool,
    ) {
        caller.require_auth();
        Self::require_compliance_officer(&env, &caller);

        let mut profile: ThirdPartyRisk = env
            .storage()
            .instance()
            .get(&AntiCorruptionKey::ThirdPartyRisk(third_party_id.clone()))
            .unwrap_or_else(|| {
                panic_with_error!(&env, AntiCorruptionError::ThirdPartyRiskNotAssessed)
            });

        profile.due_diligence_completed = true;
        profile.due_diligence_date = env.ledger().timestamp();
        profile.beneficial_owners_disclosed = beneficial_owners_disclosed;

        env.storage()
            .instance()
            .set(&AntiCorruptionKey::ThirdPartyRisk(third_party_id.clone()), &profile);

        env.events().publish(
            (Symbol::new(&env, "due_diligence_completed"),),
            (third_party_id, beneficial_owners_disclosed),
        );
    }

    /// Get third-party risk profile
    pub fn get_third_party_risk(env: Env, third_party: Address) -> ThirdPartyRisk {
        if let Some(risk_id) = env
            .storage()
            .instance()
            .get::<_, BytesN<32>>(&AntiCorruptionKey::ThirdPartyByAddress(third_party.clone()))
        {
            env.storage()
                .instance()
                .get(&AntiCorruptionKey::ThirdPartyRisk(risk_id))
                .unwrap_or_else(|| {
                    panic_with_error!(&env, AntiCorruptionError::ThirdPartyRiskNotAssessed)
                })
        } else {
            panic_with_error!(&env, AntiCorruptionError::ThirdPartyRiskNotAssessed)
        }
    }

    // ── Transaction Monitoring ───────────────────────────────────────────

    /// Monitor and screen transaction
    pub fn monitor_transaction(
        env: Env,
        caller: Address,
        from: Address,
        to: Address,
        tx_type: u32,
        amount: u64,
        currency: Bytes,
        description: Bytes,
    ) -> BytesN<32> {
        caller.require_auth();

        let tx_id = Self::compute_transaction_id(&env, &from, &to, amount);
        let now = env.ledger().timestamp();

        let mut risk_flags: Vec<Bytes> = Vec::new(&env);
        let mut status = 1u32; // Approved by default

        // Screening checks
        if Self::is_sanctions_match(&env, &to) {
            risk_flags.push_back(Bytes::from_slice(&env, b"SANCTIONS_MATCH"));
            status = 2; // Rejected
            panic_with_error!(&env, AntiCorruptionError::SanctionsListMatch);
        }

        if Self::is_pep(&env, &to) {
            risk_flags.push_back(Bytes::from_slice(&env, b"PEP_DETECTED"));
        }

        if Self::is_high_risk_jurisdiction(&env, &currency) {
            risk_flags.push_back(Bytes::from_slice(&env, b"HIGH_RISK_JURISDICTION"));
        }

        // Gift limit screening
        if tx_type == 2 && amount > 500u64 {
            risk_flags.push_back(Bytes::from_slice(&env, b"GIFT_LIMIT_EXCEEDED"));
            status = 0; // Pending review
        }

        let transaction = MonitoredTransaction {
            id: tx_id.clone(),
            from: from.clone(),
            to: to.clone(),
            tx_type,
            amount,
            currency: currency.clone(),
            description: description.clone(),
            tx_date: now,
            risk_flags: risk_flags.clone(),
            status,
            approval_date: if status == 1 { now } else { 0 },
            approved_by: if status == 1 { caller.clone() } else { Address::generate(&env) },
            tx_hash: env
                .crypto()
                .sha256(&Self::pack_transaction_data(&env, &from, &to, amount, now))
                .into(),
        };

        env.storage()
            .instance()
            .set(&AntiCorruptionKey::MonitoredTransaction(tx_id.clone()), &transaction);

        env.storage().instance().set(
            &AntiCorruptionKey::TransactionsByInitiator(from.clone()),
            &tx_id,
        );

        let count: u32 = env
            .storage()
            .instance()
            .get(&AntiCorruptionKey::TransactionCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&AntiCorruptionKey::TransactionCount, &(count + 1));

        env.events().publish(
            (Symbol::new(&env, "transaction_monitored"),),
            (tx_id.clone(), from, to, status),
        );

        tx_id
    }

    /// Approve pending transaction
    pub fn approve_transaction(env: Env, caller: Address, tx_id: BytesN<32>) {
        caller.require_auth();
        Self::require_compliance_officer(&env, &caller);

        let mut transaction: MonitoredTransaction = env
            .storage()
            .instance()
            .get(&AntiCorruptionKey::MonitoredTransaction(tx_id.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, AntiCorruptionError::ProhibitedTransaction));

        transaction.status = 1; // Approved
        transaction.approval_date = env.ledger().timestamp();
        transaction.approved_by = caller.clone();

        env.storage()
            .instance()
            .set(&AntiCorruptionKey::MonitoredTransaction(tx_id.clone()), &transaction);

        env.events().publish(
            (Symbol::new(&env, "transaction_approved"),),
            (tx_id, caller),
        );
    }

    /// Get monitored transaction
    pub fn get_transaction(env: Env, tx_id: BytesN<32>) -> MonitoredTransaction {
        env.storage()
            .instance()
            .get(&AntiCorruptionKey::MonitoredTransaction(tx_id))
            .unwrap_or_else(|| panic_with_error!(&env, AntiCorruptionError::ProhibitedTransaction))
    }

    // ── Whistleblower System ─────────────────────────────────────────────

    /// Submit anonymous whistleblower report (confidential)
    pub fn submit_whistleblower_report(
        env: Env,
        reporter: Address,
        title: Bytes,
        description_encrypted: Bytes,
        reporter_contact_encrypted: Bytes,
        confidentiality_level: u32,
    ) -> BytesN<32> {
        reporter.require_auth();

        let report_id = Self::compute_report_id(&env, &reporter, &title);
        let now = env.ledger().timestamp();

        let report = WhistleblowerReport {
            id: report_id.clone(),
            reporter: reporter.clone(),
            title: title.clone(),
            description_encrypted: description_encrypted.clone(),
            reported_at: now,
            status: 1, // Acknowledged
            investigator: Address::generate(&env),
            findings_encrypted: Bytes::new(&env),
            corrective_actions: Bytes::new(&env),
            reporter_contact_encrypted,
            confidentiality_level,
            report_hash: env
                .crypto()
                .sha256(&Self::pack_report_data(&env, &title, now))
                .into(),
        };

        env.storage()
            .instance()
            .set(&AntiCorruptionKey::WhistleblowerReport(report_id.clone()), &report);

        env.storage().instance().set(
            &AntiCorruptionKey::WhistleblowerByReporter(reporter.clone()),
            &report_id,
        );

        let count: u32 = env
            .storage()
            .instance()
            .get(&AntiCorruptionKey::WhistleblowerCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&AntiCorruptionKey::WhistleblowerCount, &(count + 1));

        env.events().publish(
            (Symbol::new(&env, "whistleblower_report_submitted"),),
            (report_id.clone(), confidentiality_level),
        );

        report_id
    }

    /// Assign investigator to whistleblower report
    pub fn assign_investigator(
        env: Env,
        caller: Address,
        report_id: BytesN<32>,
        investigator: Address,
    ) {
        caller.require_auth();
        Self::require_compliance_officer(&env, &caller);

        let mut report: WhistleblowerReport = env
            .storage()
            .instance()
            .get(&AntiCorruptionKey::WhistleblowerReport(report_id.clone()))
            .unwrap_or_else(|| {
                panic_with_error!(&env, AntiCorruptionError::WhistleblowerReportSealed)
            });

        report.status = 3; // InProgress
        report.investigator = investigator.clone();

        env.storage()
            .instance()
            .set(&AntiCorruptionKey::WhistleblowerReport(report_id.clone()), &report);

        env.events().publish(
            (Symbol::new(&env, "investigator_assigned"),),
            (report_id, investigator),
        );
    }

    /// Complete whistleblower investigation
    pub fn complete_investigation(
        env: Env,
        caller: Address,
        report_id: BytesN<32>,
        findings_encrypted: Bytes,
        corrective_actions: Bytes,
    ) {
        caller.require_auth();
        Self::require_compliance_officer(&env, &caller);

        let mut report: WhistleblowerReport = env
            .storage()
            .instance()
            .get(&AntiCorruptionKey::WhistleblowerReport(report_id.clone()))
            .unwrap_or_else(|| {
                panic_with_error!(&env, AntiCorruptionError::WhistleblowerReportSealed)
            });

        report.status = 4; // Concluded
        report.findings_encrypted = findings_encrypted;
        report.corrective_actions = corrective_actions;

        env.storage()
            .instance()
            .set(&AntiCorruptionKey::WhistleblowerReport(report_id.clone()), &report);

        env.events().publish(
            (Symbol::new(&env, "investigation_concluded"),),
            (report_id, caller),
        );
    }

    /// Get whistleblower report (compliance officer only)
    pub fn get_whistleblower_report(
        env: Env,
        caller: Address,
        report_id: BytesN<32>,
    ) -> WhistleblowerReport {
        caller.require_auth();
        Self::require_compliance_officer(&env, &caller);

        env.storage()
            .instance()
            .get(&AntiCorruptionKey::WhistleblowerReport(report_id))
            .unwrap_or_else(|| {
                panic_with_error!(&env, AntiCorruptionError::WhistleblowerReportSealed)
            })
    }

    // ── Incident Reporting ───────────────────────────────────────────────

    /// Report compliance incident
    pub fn report_incident(
        env: Env,
        caller: Address,
        incident_type: Bytes,
        description: Bytes,
        severity: u32,
        root_cause: Bytes,
        corrective_actions: Bytes,
        remediation_days: u32,
    ) -> BytesN<32> {
        caller.require_auth();

        let incident_id = Self::compute_incident_id(&env, &incident_type);
        let now = env.ledger().timestamp();

        let incident = ComplianceIncident {
            id: incident_id.clone(),
            incident_type: incident_type.clone(),
            description,
            detected_at: now,
            reported_by: caller.clone(),
            severity,
            status: 0, // Reported
            root_cause,
            corrective_actions,
            remediation_due_date: now + (remediation_days as u64 * 86400),
            incident_hash: env
                .crypto()
                .sha256(&Self::pack_incident_data(&env, &incident_type, now))
                .into(),
        };

        env.storage()
            .instance()
            .set(&AntiCorruptionKey::ComplianceIncident(incident_id.clone()), &incident);

        let count: u32 = env
            .storage()
            .instance()
            .get(&AntiCorruptionKey::IncidentCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&AntiCorruptionKey::IncidentCount, &(count + 1));

        if severity >= 3 {
            // High or Critical severity
            let violation_count: u32 = env
                .storage()
                .instance()
                .get(&AntiCorruptionKey::ViolationCount)
                .unwrap_or(0);
            env.storage()
                .instance()
                .set(&AntiCorruptionKey::ViolationCount, &(violation_count + 1));
        }

        env.events().publish(
            (Symbol::new(&env, "compliance_incident_reported"),),
            (incident_id.clone(), incident_type, severity),
        );

        incident_id
    }

    /// Get compliance incident
    pub fn get_incident(env: Env, incident_id: BytesN<32>) -> ComplianceIncident {
        env.storage()
            .instance()
            .get(&AntiCorruptionKey::ComplianceIncident(incident_id))
            .unwrap_or_else(|| panic_with_error!(&env, AntiCorruptionError::ComplianceViolation))
    }

    // ── High-Risk Jurisdiction Management ────────────────────────────────

    /// Add high-risk jurisdiction
    pub fn add_high_risk_jurisdiction(
        env: Env,
        caller: Address,
        country_code: Bytes,
        country_name: Bytes,
        risk_factors: Vec<Bytes>,
        restriction_level: u32,
    ) {
        caller.require_auth();
        Self::require_compliance_officer(&env, &caller);

        let jurisdiction = HighRiskJurisdiction {
            country_code: country_code.clone(),
            country_name,
            risk_factors,
            restriction_level,
            updated_at: env.ledger().timestamp(),
        };

        env.storage()
            .instance()
            .set(&AntiCorruptionKey::HighRiskJurisdiction(country_code.clone()), &jurisdiction);

        env.events().publish(
            (Symbol::new(&env, "high_risk_jurisdiction_added"),),
            (country_code,),
        );
    }

    /// Check if jurisdiction is high-risk
    pub fn is_high_risk_jurisdiction_check(env: Env, country_code: Bytes) -> bool {
        env.storage()
            .instance()
            .has(&AntiCorruptionKey::HighRiskJurisdiction(country_code))
    }

    // ── Compliance Statistics ────────────────────────────────────────────

    /// Get compliance statistics
    pub fn get_compliance_stats(env: Env) -> (u32, u32, u32, u32) {
        let total_assessments: u32 = env
            .storage()
            .instance()
            .get(&AntiCorruptionKey::AssessmentCount)
            .unwrap_or(0);
        let total_training: u32 = env
            .storage()
            .instance()
            .get(&AntiCorruptionKey::TrainingCount)
            .unwrap_or(0);
        let total_violations: u32 = env
            .storage()
            .instance()
            .get(&AntiCorruptionKey::ViolationCount)
            .unwrap_or(0);
        let total_incidents: u32 = env
            .storage()
            .instance()
            .get(&AntiCorruptionKey::IncidentCount)
            .unwrap_or(0);

        (total_assessments, total_training, total_violations, total_incidents)
    }

    // ── Private Helper Functions ─────────────────────────────────────────

    fn require_compliance_officer(env: &Env, caller: &Address) {
        let officer: Address = env
            .storage()
            .instance()
            .get(&AntiCorruptionKey::ComplianceOfficer)
            .unwrap();
        if caller != &officer {
            panic_with_error!(env, AntiCorruptionError::UnauthorizedWhistleblowerAccess);
        }
    }

    fn is_compliance_officer(env: &Env, caller: &Address) -> bool {
        if let Some(officer) = env
            .storage()
            .instance()
            .get::<_, Address>(&AntiCorruptionKey::ComplianceOfficer)
        {
            caller == &officer
        } else {
            false
        }
    }

    fn is_high_risk_jurisdiction(env: &Env, country_code: &Bytes) -> bool {
        env.storage()
            .instance()
            .has(&AntiCorruptionKey::HighRiskJurisdiction(country_code.clone()))
    }

    fn is_sanctions_match(env: &Env, party: &Address) -> bool {
        env.storage()
            .instance()
            .get::<_, bool>(&AntiCorruptionKey::SanctionsMatch(party.clone()))
            .unwrap_or(false)
    }

    fn is_pep(env: &Env, party: &Address) -> bool {
        env.storage()
            .instance()
            .get::<_, bool>(&AntiCorruptionKey::PEPRegistry(party.clone()))
            .unwrap_or(false)
    }

    fn compute_policy_id(env: &Env, title: &Bytes, content: Bytes) -> BytesN<32> {
        let mut preimage = Bytes::new(env);
        preimage.append(title);
        preimage.append(&content);
        preimage.append(&Self::u64_to_bytes(env, env.ledger().timestamp()));
        env.crypto().sha256(&preimage).into()
    }

    fn compute_assessment_id(env: &Env, subject: &Address) -> BytesN<32> {
        let mut preimage = Bytes::new(env);
        preimage.append(&subject.to_string().to_bytes());
        preimage.append(&Self::u64_to_bytes(env, env.ledger().timestamp()));
        env.crypto().sha256(&preimage).into()
    }

    fn compute_training_id(env: &Env, employee: &Address, training_type: u32) -> BytesN<32> {
        let mut preimage = Bytes::new(env);
        preimage.append(&employee.to_string().to_bytes());
        preimage.append(&Self::u32_to_bytes(env, training_type));
        preimage.append(&Self::u64_to_bytes(env, env.ledger().timestamp()));
        env.crypto().sha256(&preimage).into()
    }

    fn compute_third_party_id(env: &Env, party: &Address, name: &Bytes) -> BytesN<32> {
        let mut preimage = Bytes::new(env);
        preimage.append(&party.to_string().to_bytes());
        preimage.append(name);
        env.crypto().sha256(&preimage).into()
    }

    fn compute_transaction_id(env: &Env, from: &Address, to: &Address, amount: u64) -> BytesN<32> {
        let mut preimage = Bytes::new(env);
        preimage.append(&from.to_string().to_bytes());
        preimage.append(&to.to_string().to_bytes());
        preimage.append(&Self::u64_to_bytes(env, amount));
        preimage.append(&Self::u64_to_bytes(env, env.ledger().timestamp()));
        env.crypto().sha256(&preimage).into()
    }

    fn compute_report_id(env: &Env, reporter: &Address, title: &Bytes) -> BytesN<32> {
        let mut preimage = Bytes::new(env);
        preimage.append(&reporter.to_string().to_bytes());
        preimage.append(title);
        preimage.append(&Self::u64_to_bytes(env, env.ledger().timestamp()));
        env.crypto().sha256(&preimage).into()
    }

    fn compute_incident_id(env: &Env, incident_type: &Bytes) -> BytesN<32> {
        let mut preimage = Bytes::new(env);
        preimage.append(incident_type);
        preimage.append(&Self::u64_to_bytes(env, env.ledger().timestamp()));
        env.crypto().sha256(&preimage).into()
    }

    fn pack_assessment_data(env: &Env, subject: &Address, risk_level: u32, timestamp: u64) -> Bytes {
        let mut data = Bytes::new(env);
        data.append(&subject.to_string().to_bytes());
        data.append(&Self::u32_to_bytes(env, risk_level));
        data.append(&Self::u64_to_bytes(env, timestamp));
        data
    }

    fn pack_training_data(env: &Env, employee: &Address, training_type: u32, timestamp: u64) -> Bytes {
        let mut data = Bytes::new(env);
        data.append(&employee.to_string().to_bytes());
        data.append(&Self::u32_to_bytes(env, training_type));
        data.append(&Self::u64_to_bytes(env, timestamp));
        data
    }

    fn pack_third_party_data(
        env: &Env,
        party: &Address,
        name: &Bytes,
        risk_level: u32,
    ) -> Bytes {
        let mut data = Bytes::new(env);
        data.append(&party.to_string().to_bytes());
        data.append(name);
        data.append(&Self::u32_to_bytes(env, risk_level));
        data
    }

    fn pack_transaction_data(env: &Env, from: &Address, to: &Address, amount: u64, timestamp: u64) -> Bytes {
        let mut data = Bytes::new(env);
        data.append(&from.to_string().to_bytes());
        data.append(&to.to_string().to_bytes());
        data.append(&Self::u64_to_bytes(env, amount));
        data.append(&Self::u64_to_bytes(env, timestamp));
        data
    }

    fn pack_report_data(env: &Env, title: &Bytes, timestamp: u64) -> Bytes {
        let mut data = Bytes::new(env);
        data.append(title);
        data.append(&Self::u64_to_bytes(env, timestamp));
        data
    }

    fn pack_incident_data(env: &Env, incident_type: &Bytes, timestamp: u64) -> Bytes {
        let mut data = Bytes::new(env);
        data.append(incident_type);
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

    fn u32_to_bytes(env: &Env, v: u32) -> Bytes {
        bytes!(
            env,
            [
                (v & 0xff) as u8,
                ((v >> 8) & 0xff) as u8,
                ((v >> 16) & 0xff) as u8,
                ((v >> 24) & 0xff) as u8,
            ]
        )
    }
}

#[cfg(test)]
mod tests;
