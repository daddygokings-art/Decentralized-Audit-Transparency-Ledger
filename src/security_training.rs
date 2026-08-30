//! Security Training and Awareness Program for Decentralized Audit Ledger
//!
//! Provides on-chain and operational capabilities for tracking developer security training:
//! - Secure coding standards (Soroban SDK, memory safety, arithmetic invariants)
//! - Threat modeling (STRIDE, DREAD, trust boundaries)
//! - Incident response runbooks & cryptographic emergency procedures
//! - Regulatory compliance (SOC 2, ISO 27001, MiCA, GDPR)
//! - Phishing simulation campaign tracking & resiliency metrics
//! - Security Champion network governance and activity auditing

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Bytes, BytesN,
    Env, Symbol, Vec,
};

// ============================================================================
// Data Structures
// ============================================================================

/// Topics covered in the security training program
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum TrainingTopic {
    SecureCoding = 0,
    ThreatModeling = 1,
    IncidentResponse = 2,
    Compliance = 3,
    SmartContractSecurity = 4,
}

/// Difficulty and proficiency level of a training module
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum TrainingLevel {
    Foundational = 0,
    Intermediate = 1,
    Advanced = 2,
    Expert = 3,
}

/// A registered security training module
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrainingModule {
    pub id: u32,
    pub title: Bytes,
    pub topic: TrainingTopic,
    pub level: TrainingLevel,
    pub version: u32,
    pub duration_minutes: u32,
    pub passing_score: u32,
    pub is_mandatory: bool,
    pub validity_days: u32,
    pub is_active: bool,
}

/// Record of a developer completing a training module
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeveloperTrainingRecord {
    pub developer: Address,
    pub module_id: u32,
    pub completion_timestamp: u64,
    pub score: u32,
    pub passed: bool,
    pub expires_at: u64,
    pub certificate_hash: BytesN<32>,
    pub attempt_number: u32,
}

/// Status of a phishing simulation campaign
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum SimulationStatus {
    Scheduled = 0,
    Active = 1,
    Completed = 2,
    Cancelled = 3,
}

/// Outcome / action taken by a target in a phishing simulation
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum PhishingAction {
    ReportedPhish = 0,
    Ignored = 1,
    ClickedLink = 2,
    SubmittedCredentials = 3,
}

/// Phishing simulation campaign record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhishingSimulation {
    pub simulation_id: u32,
    pub campaign_name: Bytes,
    pub launch_timestamp: u64,
    pub end_timestamp: u64,
    pub total_targets: u32,
    pub reported_count: u32,
    pub clicked_count: u32,
    pub compromised_count: u32,
    pub status: SimulationStatus,
}

/// Individual phishing simulation interaction record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhishingReportRecord {
    pub developer: Address,
    pub simulation_id: u32,
    pub action: PhishingAction,
    pub timestamp: u64,
    pub report_latency_seconds: u32,
}

/// Security Champion tier
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum ChampionTier {
    Associate = 0,
    Practitioner = 1,
    Lead = 2,
    Fellow = 3,
}

/// Security Champion profile
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecurityChampion {
    pub champion: Address,
    pub department: Symbol,
    pub appointed_at: u64,
    pub tier: ChampionTier,
    pub reviews_completed: u32,
    pub threat_models_conducted: u32,
    pub active: bool,
}

/// Overall program metrics for executive and compliance reporting
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecurityProgramMetrics {
    pub total_trained_devs: u32,
    pub mandatory_compliance_pct: u32,
    pub total_simulations: u32,
    pub avg_phishing_report_pct: u32,
    pub avg_phishing_click_pct: u32,
    pub total_champions: u32,
}

// ============================================================================
// Storage Keys
// ============================================================================

#[contracttype]
pub enum SecurityTrainingKey {
    Owner,
    Module(u32),
    AllModuleIds,
    DeveloperRecords(Address),
    AllDevelopers,
    PhishingCampaign(u32),
    AllSimulationIds,
    PhishingRecord(Address, u32),
    Champion(Address),
    AllChampions,
}

// ============================================================================
// Contract Errors
// ============================================================================

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum SecurityTrainingError {
    Unauthorized = 1,
    ModuleNotFound = 2,
    ModuleInactive = 3,
    InvalidScore = 4,
    CampaignNotFound = 5,
    CampaignClosed = 6,
    ChampionNotFound = 7,
    AlreadyChampion = 8,
    DuplicateModule = 9,
}

// ============================================================================
// Contract Implementation
// ============================================================================

#[contract]
pub struct SecurityTrainingProgram;

#[contractimpl]
impl SecurityTrainingProgram {
    /// Initialize security training program with contract owner
    pub fn initialize(env: Env, owner: Address) {
        owner.require_auth();

        if env.storage().instance().has(&SecurityTrainingKey::Owner) {
            panic_with_error!(&env, SecurityTrainingError::Unauthorized);
        }

        env.storage().instance().set(&SecurityTrainingKey::Owner, &owner);
        let empty_module_ids: Vec<u32> = Vec::new(&env);
        env.storage().instance().set(&SecurityTrainingKey::AllModuleIds, &empty_module_ids);
        let empty_devs: Vec<Address> = Vec::new(&env);
        env.storage().instance().set(&SecurityTrainingKey::AllDevelopers, &empty_devs);
        let empty_sims: Vec<u32> = Vec::new(&env);
        env.storage().instance().set(&SecurityTrainingKey::AllSimulationIds, &empty_sims);
        let empty_champions: Vec<Address> = Vec::new(&env);
        env.storage().instance().set(&SecurityTrainingKey::AllChampions, &empty_champions);
    }

    /// Register a new training module (admin-only)
    pub fn register_module(env: Env, admin: Address, module: TrainingModule) {
        admin.require_auth();
        Self::require_owner(&env, &admin);

        let mut module_ids: Vec<u32> = env
            .storage()
            .instance()
            .get(&SecurityTrainingKey::AllModuleIds)
            .unwrap_or_else(|| Vec::new(&env));

        for id in module_ids.iter() {
            if id == module.id {
                panic_with_error!(&env, SecurityTrainingError::DuplicateModule);
            }
        }

        module_ids.push_back(module.id);
        env.storage().instance().set(&SecurityTrainingKey::AllModuleIds, &module_ids);
        env.storage().instance().set(&SecurityTrainingKey::Module(module.id), &module);

        env.events().publish(
            (Symbol::new(&env, "sec_mod_created"), module.id),
            (module.topic, module.is_mandatory),
        );
    }

    /// Update an existing training module
    pub fn update_module(env: Env, admin: Address, module: TrainingModule) {
        admin.require_auth();
        Self::require_owner(&env, &admin);

        if !env.storage().instance().has(&SecurityTrainingKey::Module(module.id)) {
            panic_with_error!(&env, SecurityTrainingError::ModuleNotFound);
        }

        env.storage().instance().set(&SecurityTrainingKey::Module(module.id), &module);

        env.events().publish(
            (Symbol::new(&env, "sec_mod_updated"), module.id),
            (module.version, module.is_active),
        );
    }

    /// Get module by ID
    pub fn get_module(env: Env, module_id: u32) -> TrainingModule {
        env.storage()
            .instance()
            .get(&SecurityTrainingKey::Module(module_id))
            .unwrap_or_else(|| panic_with_error!(&env, SecurityTrainingError::ModuleNotFound))
    }

    /// List all registered module IDs
    pub fn get_all_module_ids(env: Env) -> Vec<u32> {
        env.storage()
            .instance()
            .get(&SecurityTrainingKey::AllModuleIds)
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Record a developer's completion of a training module
    pub fn record_training_completion(
        env: Env,
        developer: Address,
        module_id: u32,
        score: u32,
        certificate_hash: BytesN<32>,
    ) -> DeveloperTrainingRecord {
        developer.require_auth();

        if score > 100 {
            panic_with_error!(&env, SecurityTrainingError::InvalidScore);
        }

        let module = Self::get_module(env.clone(), module_id);
        if !module.is_active {
            panic_with_error!(&env, SecurityTrainingError::ModuleInactive);
        }

        let now = env.ledger().timestamp();
        let passed = score >= module.passing_score;
        let validity_seconds = (module.validity_days as u64) * 86400;
        let expires_at = if passed && validity_seconds > 0 {
            now + validity_seconds
        } else {
            0
        };

        let mut dev_records: Vec<DeveloperTrainingRecord> = env
            .storage()
            .instance()
            .get(&SecurityTrainingKey::DeveloperRecords(developer.clone()))
            .unwrap_or_else(|| Vec::new(&env));

        let mut attempt_number = 1u32;
        for rec in dev_records.iter() {
            if rec.module_id == module_id {
                attempt_number += 1;
            }
        }

        let new_record = DeveloperTrainingRecord {
            developer: developer.clone(),
            module_id,
            completion_timestamp: now,
            score,
            passed,
            expires_at,
            certificate_hash: certificate_hash.clone(),
            attempt_number,
        };

        dev_records.push_back(new_record.clone());
        env.storage()
            .instance()
            .set(&SecurityTrainingKey::DeveloperRecords(developer.clone()), &dev_records);

        // Register developer in all-developers list if first record
        let mut all_devs: Vec<Address> = env
            .storage()
            .instance()
            .get(&SecurityTrainingKey::AllDevelopers)
            .unwrap_or_else(|| Vec::new(&env));

        let mut exists = false;
        for d in all_devs.iter() {
            if d == developer {
                exists = true;
                break;
            }
        }
        if !exists {
            all_devs.push_back(developer.clone());
            env.storage().instance().set(&SecurityTrainingKey::AllDevelopers, &all_devs);
        }

        env.events().publish(
            (Symbol::new(&env, "sec_train_completed"), developer, module_id),
            (score, passed, expires_at),
        );

        new_record
    }

    /// Retrieve all completion records for a developer
    pub fn get_developer_records(env: Env, developer: Address) -> Vec<DeveloperTrainingRecord> {
        env.storage()
            .instance()
            .get(&SecurityTrainingKey::DeveloperRecords(developer))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Check if a developer has active passing certifications for all mandatory modules
    pub fn is_developer_compliant(env: Env, developer: Address) -> bool {
        let module_ids = Self::get_all_module_ids(env.clone());
        let records = Self::get_developer_records(env.clone(), developer);
        let now = env.ledger().timestamp();

        for m_id in module_ids.iter() {
            let module = Self::get_module(env.clone(), m_id);
            if module.is_mandatory && module.is_active {
                let mut module_passed = false;
                for rec in records.iter() {
                    if rec.module_id == m_id && rec.passed && rec.expires_at > now {
                        module_passed = true;
                        break;
                    }
                }
                if !module_passed {
                    return false;
                }
            }
        }

        true
    }

    /// Create and launch a phishing simulation campaign
    pub fn create_phishing_campaign(
        env: Env,
        admin: Address,
        simulation_id: u32,
        campaign_name: Bytes,
        launch_timestamp: u64,
        end_timestamp: u64,
        total_targets: u32,
    ) -> PhishingSimulation {
        admin.require_auth();
        Self::require_owner(&env, &admin);

        let sim = PhishingSimulation {
            simulation_id,
            campaign_name,
            launch_timestamp,
            end_timestamp,
            total_targets,
            reported_count: 0,
            clicked_count: 0,
            compromised_count: 0,
            status: SimulationStatus::Active,
        };

        let mut sim_ids: Vec<u32> = env
            .storage()
            .instance()
            .get(&SecurityTrainingKey::AllSimulationIds)
            .unwrap_or_else(|| Vec::new(&env));
        sim_ids.push_back(simulation_id);
        env.storage().instance().set(&SecurityTrainingKey::AllSimulationIds, &sim_ids);
        env.storage().instance().set(&SecurityTrainingKey::PhishingCampaign(simulation_id), &sim);

        env.events().publish(
            (Symbol::new(&env, "phish_sim_created"), simulation_id),
            (launch_timestamp, total_targets),
        );

        sim
    }

    /// Record a target developer's action in response to a phishing simulation
    pub fn record_phishing_action(
        env: Env,
        developer: Address,
        simulation_id: u32,
        action: PhishingAction,
        latency_seconds: u32,
    ) {
        let mut sim: PhishingSimulation = env
            .storage()
            .instance()
            .get(&SecurityTrainingKey::PhishingCampaign(simulation_id))
            .unwrap_or_else(|| panic_with_error!(&env, SecurityTrainingError::CampaignNotFound));

        if sim.status != SimulationStatus::Active {
            panic_with_error!(&env, SecurityTrainingError::CampaignClosed);
        }

        match action {
            PhishingAction::ReportedPhish => {
                sim.reported_count += 1;
            }
            PhishingAction::ClickedLink => {
                sim.clicked_count += 1;
            }
            PhishingAction::SubmittedCredentials => {
                sim.clicked_count += 1;
                sim.compromised_count += 1;
            }
            PhishingAction::Ignored => {}
        }

        env.storage().instance().set(&SecurityTrainingKey::PhishingCampaign(simulation_id), &sim);

        let now = env.ledger().timestamp();
        let report_record = PhishingReportRecord {
            developer: developer.clone(),
            simulation_id,
            action,
            timestamp: now,
            report_latency_seconds: latency_seconds,
        };

        env.storage().instance().set(
            &SecurityTrainingKey::PhishingRecord(developer.clone(), simulation_id),
            &report_record,
        );

        env.events().publish(
            (Symbol::new(&env, "phish_action_logged"), developer, simulation_id),
            (action, latency_seconds),
        );
    }

    /// Retrieve a phishing simulation campaign details
    pub fn get_phishing_simulation(env: Env, simulation_id: u32) -> PhishingSimulation {
        env.storage()
            .instance()
            .get(&SecurityTrainingKey::PhishingCampaign(simulation_id))
            .unwrap_or_else(|| panic_with_error!(&env, SecurityTrainingError::CampaignNotFound))
    }

    /// Appoint a developer as a Security Champion
    pub fn appoint_security_champion(
        env: Env,
        admin: Address,
        champion: Address,
        department: Symbol,
        tier: ChampionTier,
    ) {
        admin.require_auth();
        Self::require_owner(&env, &admin);

        if env.storage().instance().has(&SecurityTrainingKey::Champion(champion.clone())) {
            panic_with_error!(&env, SecurityTrainingError::AlreadyChampion);
        }

        let now = env.ledger().timestamp();
        let champ = SecurityChampion {
            champion: champion.clone(),
            department,
            appointed_at: now,
            tier,
            reviews_completed: 0,
            threat_models_conducted: 0,
            active: true,
        };

        let mut all_champs: Vec<Address> = env
            .storage()
            .instance()
            .get(&SecurityTrainingKey::AllChampions)
            .unwrap_or_else(|| Vec::new(&env));
        all_champs.push_back(champion.clone());
        env.storage().instance().set(&SecurityTrainingKey::AllChampions, &all_champs);
        env.storage().instance().set(&SecurityTrainingKey::Champion(champion.clone()), &champ);

        env.events().publish(
            (Symbol::new(&env, "champion_appointed"), champion),
            (department, tier),
        );
    }

    /// Promote or update tier for an existing Security Champion
    pub fn promote_security_champion(
        env: Env,
        admin: Address,
        champion: Address,
        new_tier: ChampionTier,
    ) {
        admin.require_auth();
        Self::require_owner(&env, &admin);

        let mut champ: SecurityChampion = env
            .storage()
            .instance()
            .get(&SecurityTrainingKey::Champion(champion.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, SecurityTrainingError::ChampionNotFound));

        champ.tier = new_tier;
        env.storage().instance().set(&SecurityTrainingKey::Champion(champion.clone()), &champ);

        env.events().publish(
            (Symbol::new(&env, "champion_promoted"), champion),
            new_tier,
        );
    }

    /// Log a security activity performed by a Security Champion (e.g. PR security review, threat model)
    pub fn log_champion_activity(env: Env, champion: Address, activity_type: Symbol) {
        champion.require_auth();

        let mut champ: SecurityChampion = env
            .storage()
            .instance()
            .get(&SecurityTrainingKey::Champion(champion.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, SecurityTrainingError::ChampionNotFound));

        if !champ.active {
            panic_with_error!(&env, SecurityTrainingError::Unauthorized);
        }

        if activity_type == Symbol::new(&env, "pr_review") {
            champ.reviews_completed += 1;
        } else if activity_type == Symbol::new(&env, "threat_model") {
            champ.threat_models_conducted += 1;
        }

        env.storage().instance().set(&SecurityTrainingKey::Champion(champion.clone()), &champ);

        env.events().publish(
            (Symbol::new(&env, "champion_act_logged"), champion),
            activity_type,
        );
    }

    /// Get Security Champion profile
    pub fn get_security_champion(env: Env, champion: Address) -> SecurityChampion {
        env.storage()
            .instance()
            .get(&SecurityTrainingKey::Champion(champion))
            .unwrap_or_else(|| panic_with_error!(&env, SecurityTrainingError::ChampionNotFound))
    }

    /// Calculate aggregated metrics for SOC 2 / ISO 27001 compliance reporting
    pub fn get_program_metrics(env: Env) -> SecurityProgramMetrics {
        let all_devs: Vec<Address> = env
            .storage()
            .instance()
            .get(&SecurityTrainingKey::AllDevelopers)
            .unwrap_or_else(|| Vec::new(&env));
        let total_trained_devs = all_devs.len();

        let mut compliant_devs = 0u32;
        for dev in all_devs.iter() {
            if Self::is_developer_compliant(env.clone(), dev) {
                compliant_devs += 1;
            }
        }

        let mandatory_compliance_pct = if total_trained_devs > 0 {
            (compliant_devs * 100) / total_trained_devs
        } else {
            100
        };

        let sim_ids: Vec<u32> = env
            .storage()
            .instance()
            .get(&SecurityTrainingKey::AllSimulationIds)
            .unwrap_or_else(|| Vec::new(&env));
        let total_simulations = sim_ids.len();

        let mut total_targets = 0u32;
        let mut total_reports = 0u32;
        let mut total_clicks = 0u32;

        for s_id in sim_ids.iter() {
            if let Some(sim) = env.storage().instance().get::<_, PhishingSimulation>(&SecurityTrainingKey::PhishingCampaign(s_id)) {
                total_targets += sim.total_targets;
                total_reports += sim.reported_count;
                total_clicks += sim.clicked_count;
            }
        }

        let avg_phishing_report_pct = if total_targets > 0 {
            (total_reports * 100) / total_targets
        } else {
            0
        };

        let avg_phishing_click_pct = if total_targets > 0 {
            (total_clicks * 100) / total_targets
        } else {
            0
        };

        let champions: Vec<Address> = env
            .storage()
            .instance()
            .get(&SecurityTrainingKey::AllChampions)
            .unwrap_or_else(|| Vec::new(&env));
        let total_champions = champions.len();

        SecurityProgramMetrics {
            total_trained_devs,
            mandatory_compliance_pct,
            total_simulations,
            avg_phishing_report_pct,
            avg_phishing_click_pct,
            total_champions,
        }
    }

    /// Internal helper to check owner authority
    fn require_owner(env: &Env, admin: &Address) {
        let owner: Address = env
            .storage()
            .instance()
            .get(&SecurityTrainingKey::Owner)
            .unwrap_or_else(|| panic_with_error!(env, SecurityTrainingError::Unauthorized));
        if *admin != owner {
            panic_with_error!(env, SecurityTrainingError::Unauthorized);
        }
    }
}
