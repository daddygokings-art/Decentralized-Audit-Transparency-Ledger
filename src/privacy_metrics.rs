/// Contract Event Privacy Metrics and Reporting
///
/// Tracks privacy-related metrics for audit events:
/// - Data subject requests (DSR)
/// - Breach incidents
/// - DPIA completion
/// - Training completion
/// - Consent rates
/// - Data minimization effectiveness
///
/// Provides queryable metrics for off-chain privacy dashboards.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, Bytes, BytesN, Env, Symbol,
    Vec, Map, panic_with_error, log,
};

// ============================================================================
// Data Structures
// ============================================================================

/// Data subject request types (GDPR/CCPA style)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum DsrType {
    Access = 0,
    Deletion = 1,
    Rectification = 2,
    Portability = 3,
    Restriction = 4,
    Objection = 5,
}

/// A data subject request record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataSubjectRequest {
    pub id: BytesN<32>,
    pub requester: Address,
    pub dsr_type: u32,
    pub status: Symbol,
    pub created_at: u64,
    pub resolved_at: u64,
    pub metadata: Bytes,
}

/// A breach incident record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BreachIncident {
    pub id: BytesN<32>,
    pub reporter: Address,
    pub severity: u32,
    pub description: Bytes,
    pub affected_events: Vec<BytesN<32>>,
    pub reported_at: u64,
    pub resolved: bool,
    pub resolution_notes: Bytes,
}

/// DPIA (Data Protection Impact Assessment) record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DpiaRecord {
    pub id: BytesN<32>,
    pub assessor: Address,
    pub processing_activity: Bytes,
    pub risk_level: u32,
    pub mitigation_measures: Bytes,
    pub completed: bool,
    pub created_at: u64,
    pub completed_at: u64,
}

/// Training completion record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrainingRecord {
    pub id: BytesN<32>,
    pub participant: Address,
    pub training_name: Symbol,
    pub completed_at: u64,
    pub expires_at: u64,
    pub score: u32,
}

/// Consent record for data processing
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsentRecord {
    pub id: BytesN<32>,
    pub data_subject: Address,
    pub purpose: Symbol,
    pub consented: bool,
    pub recorded_at: u64,
    pub expires_at: u64,
    pub withdrawal_allowed: bool,
}

/// Data minimization effectiveness metric
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataMinimizationMetric {
    pub period_start: u64,
    pub period_end: u64,
    pub total_events: u32,
    pub events_with_minimal_metadata: u32,
    pub avg_metadata_size: u32,
    pub metadata_size_reduction_pct: u32,
}

/// Aggregated privacy metrics snapshot
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivacyMetricsSnapshot {
    pub timestamp: u64,
    pub total_dsrs: u32,
    pub pending_dsrs: u32,
    pub total_breaches: u32,
    pub open_breaches: u32,
    pub dpia_completion_rate_bps: u32,
    pub training_completion_rate_bps: u32,
    pub consent_rate_bps: u32,
    pub data_minimization_score_bps: u32,
}

// ============================================================================
// Storage Keys
// ============================================================================

#[contracttype]
pub enum PrivacyKey {
    Owner,
    DataSubjectRequest(BytesN<32>),
    AllDsrIds,
    BreachIncident(BytesN<32>),
    AllBreachIds,
    DpiaRecord(BytesN<32>),
    AllDpiaIds,
    TrainingRecord(BytesN<32>),
    AllTrainingIds,
    ConsentRecord(BytesN<32>),
    AllConsentIds,
    DataMinimizationMetric,
    PrivacyMetricsSnapshot,
    NextDsrId,
    NextBreachId,
    NextDpiaId,
    NextTrainingId,
    NextConsentId,
}

// ============================================================================
// Contract Errors
// ============================================================================

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum PrivacyError {
    Unauthorized = 1,
    DsrNotFound = 2,
    BreachNotFound = 3,
    DpiaNotFound = 4,
    TrainingNotFound = 5,
    ConsentNotFound = 6,
    InvalidDsrType = 7,
    InvalidSeverity = 8,
    InvalidRiskLevel = 9,
    InvalidConsentStatus = 10,
    DsrAlreadyResolved = 11,
    BreachAlreadyResolved = 12,
    DpiaAlreadyCompleted = 13,
    TrainingExpired = 14,
    ConsentExpired = 15,
    MetricNotFound = 16,
    InvalidSnapshot = 17,
}

// ============================================================================
// Contract Implementation
// ============================================================================

#[contract]
pub struct PrivacyMetrics;

#[contractimpl]
impl PrivacyMetrics {
    /// Initialize privacy metrics module (owner-only)
    pub fn initialize(env: Env, owner: Address) {
        owner.require_auth();

        if env.storage().instance().has(&PrivacyKey::Owner) {
            panic_with_error!(&env, PrivacyError::Unauthorized);
        }

        env.storage().instance().set(&PrivacyKey::Owner, &owner);
        env.storage().instance().set(&PrivacyKey::NextDsrId, &1u32);
        env.storage().instance().set(&PrivacyKey::NextBreachId, &1u32);
        env.storage().instance().set(&PrivacyKey::NextDpiaId, &1u32);
        env.storage().instance().set(&PrivacyKey::NextTrainingId, &1u32);
        env.storage().instance().set(&PrivacyKey::NextConsentId, &1u32);
    }

    // ========================================================================
    // Data Subject Requests (DSR)
    // ========================================================================

    /// Submit a new data subject request
    pub fn submit_dsr(
        env: Env,
        requester: Address,
        dsr_type: u32,
        metadata: Bytes,
    ) -> DataSubjectRequest {
        requester.require_auth();

        if dsr_type > 5 {
            panic_with_error!(&env, PrivacyError::InvalidDsrType);
        }

        let id = Self::get_next_dsr_id(&env);
        let id_bytes = BytesN::from_array(&env, &sha2_digest(&env, &id.to_le_bytes()));

        let dsr = DataSubjectRequest {
            id: id_bytes,
            requester: requester.clone(),
            dsr_type,
            status: Symbol::new(&env, "pending"),
            created_at: env.ledger().timestamp(),
            resolved_at: 0,
            metadata,
        };

        env.storage()
            .instance()
            .set(&PrivacyKey::DataSubjectRequest(id_bytes.clone()), &dsr);

        let mut all_ids: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&PrivacyKey::AllDsrIds)
            .unwrap_or_else(|| Vec::new(&env));
        all_ids.push_back(id_bytes.clone());
        env.storage()
            .instance()
            .set(&PrivacyKey::AllDsrIds, &all_ids);

        log!(
            &env,
            "PrivacyMetrics: DSR submitted - id={}, type={}",
            id,
            dsr_type
        );

        dsr
    }

    /// Resolve a data subject request (owner-only)
    pub fn resolve_dsr(env: Env, caller: Address, dsr_id: BytesN<32>) {
        Self::require_owner(&env, &caller);

        let mut dsr = Self::get_dsr_or_panic(&env, dsr_id.clone());

        if dsr.status == Symbol::new(&env, "resolved") {
            panic_with_error!(&env, PrivacyError::DsrAlreadyResolved);
        }

        dsr.status = Symbol::new(&env, "resolved");
        dsr.resolved_at = env.ledger().timestamp();

        env.storage()
            .instance()
            .set(&PrivacyKey::DataSubjectRequest(dsr_id), &dsr);
    }

    /// Get a data subject request by ID
    pub fn get_dsr(env: Env, dsr_id: BytesN<32>) -> DataSubjectRequest {
        Self::get_dsr_or_panic(&env, dsr_id)
    }

    /// List all data subject request IDs
    pub fn list_dsr_ids(env: Env) -> Vec<BytesN<32>> {
        env.storage()
            .instance()
            .get(&PrivacyKey::AllDsrIds)
            .unwrap_or_else(|| Vec::new(&env))
    }

    // ========================================================================
    // Breach Incidents
    // ========================================================================

    /// Report a breach incident
    pub fn report_breach(
        env: Env,
        reporter: Address,
        severity: u32,
        description: Bytes,
        affected_events: Vec<BytesN<32>>,
    ) -> BreachIncident {
        reporter.require_auth();

        if severity > 4 {
            panic_with_error!(&env, PrivacyError::InvalidSeverity);
        }

        let id = Self::get_next_breach_id(&env);
        let id_bytes = BytesN::from_array(&env, &sha2_digest(&env, &id.to_le_bytes()));

        let breach = BreachIncident {
            id: id_bytes.clone(),
            reporter: reporter.clone(),
            severity,
            description,
            affected_events,
            reported_at: env.ledger().timestamp(),
            resolved: false,
            resolution_notes: Bytes::new(&env),
        };

        env.storage()
            .instance()
            .set(&PrivacyKey::BreachIncident(id_bytes.clone()), &breach);

        let mut all_ids: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&PrivacyKey::AllBreachIds)
            .unwrap_or_else(|| Vec::new(&env));
        all_ids.push_back(id_bytes.clone());
        env.storage()
            .instance()
            .set(&PrivacyKey::AllBreachIds, &all_ids);

        log!(
            &env,
            "PrivacyMetrics: breach reported - id={}, severity={}",
            id,
            severity
        );

        breach
    }

    /// Resolve a breach incident (owner-only)
    pub fn resolve_breach(env: Env, caller: Address, breach_id: BytesN<32>, resolution_notes: Bytes) {
        Self::require_owner(&env, &caller);

        let mut breach = Self::get_breach_or_panic(&env, breach_id.clone());

        if breach.resolved {
            panic_with_error!(&env, PrivacyError::BreachAlreadyResolved);
        }

        breach.resolved = true;
        breach.resolution_notes = resolution_notes;

        env.storage()
            .instance()
            .set(&PrivacyKey::BreachIncident(breach_id), &breach);
    }

    /// Get a breach incident by ID
    pub fn get_breach(env: Env, breach_id: BytesN<32>) -> BreachIncident {
        Self::get_breach_or_panic(&env, breach_id)
    }

    /// List all breach incident IDs
    pub fn list_breach_ids(env: Env) -> Vec<BytesN<32>> {
        env.storage()
            .instance()
            .get(&PrivacyKey::AllBreachIds)
            .unwrap_or_else(|| Vec::new(&env))
    }

    // ========================================================================
    // DPIA Records
    // ========================================================================

    /// Create a DPIA record
    pub fn create_dpia(
        env: Env,
        assessor: Address,
        processing_activity: Bytes,
        risk_level: u32,
        mitigation_measures: Bytes,
    ) -> DpiaRecord {
        assessor.require_auth();

        if risk_level > 4 {
            panic_with_error!(&env, PrivacyError::InvalidRiskLevel);
        }

        let id = Self::get_next_dpia_id(&env);
        let id_bytes = BytesN::from_array(&env, &sha2_digest(&env, &id.to_le_bytes()));

        let dpia = DpiaRecord {
            id: id_bytes.clone(),
            assessor: assessor.clone(),
            processing_activity,
            risk_level,
            mitigation_measures,
            completed: false,
            created_at: env.ledger().timestamp(),
            completed_at: 0,
        };

        env.storage()
            .instance()
            .set(&PrivacyKey::DpiaRecord(id_bytes.clone()), &dpia);

        let mut all_ids: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&PrivacyKey::AllDpiaIds)
            .unwrap_or_else(|| Vec::new(&env));
        all_ids.push_back(id_bytes.clone());
        env.storage()
            .instance()
            .set(&PrivacyKey::AllDpiaIds, &all_ids);

        log!(
            &env,
            "PrivacyMetrics: DPIA created - id={}, risk_level={}",
            id,
            risk_level
        );

        dpia
    }

    /// Complete a DPIA (owner-only)
    pub fn complete_dpia(env: Env, caller: Address, dpia_id: BytesN<32>) {
        Self::require_owner(&env, &caller);

        let mut dpia = Self::get_dpia_or_panic(&env, dpia_id.clone());

        if dpia.completed {
            panic_with_error!(&env, PrivacyError::DpiaAlreadyCompleted);
        }

        dpia.completed = true;
        dpia.completed_at = env.ledger().timestamp();

        env.storage()
            .instance()
            .set(&PrivacyKey::DpiaRecord(dpia_id), &dpia);
    }

    /// Get a DPIA record by ID
    pub fn get_dpia(env: Env, dpia_id: BytesN<32>) -> DpiaRecord {
        Self::get_dpia_or_panic(&env, dpia_id)
    }

    /// List all DPIA record IDs
    pub fn list_dpia_ids(env: Env) -> Vec<BytesN<32>> {
        env.storage()
            .instance()
            .get(&PrivacyKey::AllDpiaIds)
            .unwrap_or_else(|| Vec::new(&env))
    }

    // ========================================================================
    // Training Records
    // ========================================================================

    /// Record training completion
    pub fn record_training(
        env: Env,
        participant: Address,
        training_name: Symbol,
        duration_days: u32,
        score: u32,
    ) -> TrainingRecord {
        participant.require_auth();

        let id = Self::get_next_training_id(&env);
        let id_bytes = BytesN::from_array(&env, &sha2_digest(&env, &id.to_le_bytes()));
        let now = env.ledger().timestamp();
        let duration_seconds = duration_days * 86400u64;

        let training = TrainingRecord {
            id: id_bytes.clone(),
            participant: participant.clone(),
            training_name,
            completed_at: now,
            expires_at: now + duration_seconds,
            score,
        };

        env.storage()
            .instance()
            .set(&PrivacyKey::TrainingRecord(id_bytes.clone()), &training);

        let mut all_ids: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&PrivacyKey::AllTrainingIds)
            .unwrap_or_else(|| Vec::new(&env));
        all_ids.push_back(id_bytes.clone());
        env.storage()
            .instance()
            .set(&PrivacyKey::AllTrainingIds, &all_ids);

        log!(
            &env,
            "PrivacyMetrics: training recorded - id={}, participant={}",
            id,
            participant
        );

        training
    }

    /// Get a training record by ID
    pub fn get_training(env: Env, training_id: BytesN<32>) -> TrainingRecord {
        Self::get_training_or_panic(&env, training_id)
    }

    /// List all training record IDs
    pub fn list_training_ids(env: Env) -> Vec<BytesN<32>> {
        env.storage()
            .instance()
            .get(&PrivacyKey::AllTrainingIds)
            .unwrap_or_else(|| Vec::new(&env))
    }

    // ========================================================================
    // Consent Records
    // ========================================================================

    /// Record consent for data processing
    pub fn record_consent(
        env: Env,
        data_subject: Address,
        purpose: Symbol,
        consented: bool,
        duration_days: u32,
        withdrawal_allowed: bool,
    ) -> ConsentRecord {
        data_subject.require_auth();

        let id = Self::get_next_consent_id(&env);
        let id_bytes = BytesN::from_array(&env, &sha2_digest(&env, &id.to_le_bytes()));
        let now = env.ledger().timestamp();
        let duration_seconds = duration_days * 86400u64;

        let consent = ConsentRecord {
            id: id_bytes.clone(),
            data_subject: data_subject.clone(),
            purpose,
            consented,
            recorded_at: now,
            expires_at: now + duration_seconds,
            withdrawal_allowed,
        };

        env.storage()
            .instance()
            .set(&PrivacyKey::ConsentRecord(id_bytes.clone()), &consent);

        let mut all_ids: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&PrivacyKey::AllConsentIds)
            .unwrap_or_else(|| Vec::new(&env));
        all_ids.push_back(id_bytes.clone());
        env.storage()
            .instance()
            .set(&PrivacyKey::AllConsentIds, &all_ids);

        log!(
            &env,
            "PrivacyMetrics: consent recorded - id={}, purpose={:?}, consented={}",
            id,
            purpose,
            consented
        );

        consent
    }

    /// Get a consent record by ID
    pub fn get_consent(env: Env, consent_id: BytesN<32>) -> ConsentRecord {
        Self::get_consent_or_panic(&env, consent_id)
    }

    /// List all consent record IDs
    pub fn list_consent_ids(env: Env) -> Vec<BytesN<32>> {
        env.storage()
            .instance()
            .get(&PrivacyKey::AllConsentIds)
            .unwrap_or_else(|| Vec::new(&env))
    }

    // ========================================================================
    // Data Minimization Metrics
    // ========================================================================

    /// Record data minimization effectiveness metrics
    pub fn record_data_minimization(
        env: Env,
        caller: Address,
        period_start: u64,
        period_end: u64,
        total_events: u32,
        events_with_minimal_metadata: u32,
        avg_metadata_size: u32,
        metadata_size_reduction_pct: u32,
    ) -> DataMinimizationMetric {
        Self::require_owner(&env, &caller);

        let metric = DataMinimizationMetric {
            period_start,
            period_end,
            total_events,
            events_with_minimal_metadata,
            avg_metadata_size,
            metadata_size_reduction_pct,
        };

        env.storage()
            .instance()
            .set(&PrivacyKey::DataMinimizationMetric, &metric);

        log!(
            &env,
            "PrivacyMetrics: data minimization recorded - total_events={}, minimal_events={}",
            total_events,
            events_with_minimal_metadata
        );

        metric
    }

    /// Get data minimization metrics
    pub fn get_data_minimization(env: Env) -> DataMinimizationMetric {
        env.storage()
            .instance()
            .get(&PrivacyKey::DataMinimizationMetric)
            .unwrap_or_else(|| panic_with_error!(&env, PrivacyError::MetricNotFound))
    }

    // ========================================================================
    // Privacy Dashboard / Aggregated Metrics
    // ========================================================================

    /// Compute privacy metrics snapshot for dashboard
    pub fn compute_privacy_snapshot(env: Env, caller: Address) -> PrivacyMetricsSnapshot {
        Self::require_owner(&env, &caller);

        let now = env.ledger().timestamp();

        let dsr_ids: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&PrivacyKey::AllDsrIds)
            .unwrap_or_else(|| Vec::new(&env));
        let mut total_dsrs = 0u32;
        let mut pending_dsrs = 0u32;
        for i in 0..dsr_ids.len() {
            if let Some(dsr) = env
                .storage()
                .instance()
                .get::<_, DataSubjectRequest>(&PrivacyKey::DataSubjectRequest(dsr_ids.get(i).unwrap().clone()))
            {
                total_dsrs += 1;
                if dsr.status == Symbol::new(&env, "pending") {
                    pending_dsrs += 1;
                }
            }
        }

        let breach_ids: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&PrivacyKey::AllBreachIds)
            .unwrap_or_else(|| Vec::new(&env));
        let mut total_breaches = 0u32;
        let mut open_breaches = 0u32;
        for i in 0..breach_ids.len() {
            if let Some(breach) = env
                .storage()
                .instance()
                .get::<_, BreachIncident>(&PrivacyKey::BreachIncident(breach_ids.get(i).unwrap().clone()))
            {
                total_breaches += 1;
                if !breach.resolved {
                    open_breaches += 1;
                }
            }
        }

        let dpia_ids: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&PrivacyKey::AllDpiaIds)
            .unwrap_or_else(|| Vec::new(&env));
        let mut completed_dpias = 0u32;
        for i in 0..dpia_ids.len() {
            if let Some(dpia) = env
                .storage()
                .instance()
                .get::<_, DpiaRecord>(&PrivacyKey::DpiaRecord(dpia_ids.get(i).unwrap().clone()))
            {
                if dpia.completed {
                    completed_dpias += 1;
                }
            }
        }
        let dpia_completion_rate = if dpia_ids.len() > 0 {
            (completed_dpias * 10000) / dpia_ids.len()
        } else {
            10000
        };

        let training_ids: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&PrivacyKey::AllTrainingIds)
            .unwrap_or_else(|| Vec::new(&env));
        let mut valid_trainings = 0u32;
        for i in 0..training_ids.len() {
            if let Some(training) = env
                .storage()
                .instance()
                .get::<_, TrainingRecord>(&PrivacyKey::TrainingRecord(training_ids.get(i).unwrap().clone()))
            {
                if training.expires_at == 0 || training.expires_at > now {
                    valid_trainings += 1;
                }
            }
        }
        let training_completion_rate = if training_ids.len() > 0 {
            (valid_trainings * 10000) / training_ids.len()
        } else {
            10000
        };

        let consent_ids: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&PrivacyKey::AllConsentIds)
            .unwrap_or_else(|| Vec::new(&env));
        let mut valid_consents = 0u32;
        for i in 0..consent_ids.len() {
            if let Some(consent) = env
                .storage()
                .instance()
                .get::<_, ConsentRecord>(&PrivacyKey::ConsentRecord(consent_ids.get(i).unwrap().clone()))
            {
                if consent.expires_at == 0 || consent.expires_at > now {
                    valid_consents += 1;
                }
            }
        }
        let consent_rate = if consent_ids.len() > 0 {
            (valid_consents * 10000) / consent_ids.len()
        } else {
            10000
        };

        let dm_metric = env
            .storage()
            .instance()
            .get::<_, DataMinimizationMetric>(&PrivacyKey::DataMinimizationMetric)
            .unwrap_or(DataMinimizationMetric {
                period_start: 0,
                period_end: 0,
                total_events: 0,
                events_with_minimal_metadata: 0,
                avg_metadata_size: 0,
                metadata_size_reduction_pct: 0,
            });
        let dm_score = dm_metric.metadata_size_reduction_pct;

        let snapshot = PrivacyMetricsSnapshot {
            timestamp: now,
            total_dsrs,
            pending_dsrs,
            total_breaches,
            open_breaches,
            dpia_completion_rate_bps: dpia_completion_rate,
            training_completion_rate_bps: training_completion_rate,
            consent_rate_bps: consent_rate,
            data_minimization_score_bps: dm_score,
        };

        env.storage()
            .instance()
            .set(&PrivacyKey::PrivacyMetricsSnapshot, &snapshot);

        snapshot
    }

    /// Get latest privacy metrics snapshot
    pub fn get_privacy_snapshot(env: Env) -> PrivacyMetricsSnapshot {
        env.storage()
            .instance()
            .get(&PrivacyKey::PrivacyMetricsSnapshot)
            .unwrap_or_else(|| panic_with_error!(&env, PrivacyError::InvalidSnapshot))
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    fn require_owner(env: &Env, caller: &Address) {
        let owner: Address = env
            .storage()
            .instance()
            .get(&PrivacyKey::Owner)
            .unwrap_or_else(|| panic_with_error!(env, PrivacyError::Unauthorized));
        if &owner != caller {
            panic_with_error!(env, PrivacyError::Unauthorized);
        }
    }

    fn get_dsr_or_panic(env: &Env, dsr_id: BytesN<32>) -> DataSubjectRequest {
        env.storage()
            .instance()
            .get(&PrivacyKey::DataSubjectRequest(dsr_id))
            .unwrap_or_else(|| panic_with_error!(env, PrivacyError::DsrNotFound))
    }

    fn get_breach_or_panic(env: &Env, breach_id: BytesN<32>) -> BreachIncident {
        env.storage()
            .instance()
            .get(&PrivacyKey::BreachIncident(breach_id))
            .unwrap_or_else(|| panic_with_error!(env, PrivacyError::BreachNotFound))
    }

    fn get_dpia_or_panic(env: &Env, dpia_id: BytesN<32>) -> DpiaRecord {
        env.storage()
            .instance()
            .get(&PrivacyKey::DpiaRecord(dpia_id))
            .unwrap_or_else(|| panic_with_error!(env, PrivacyError::DpiaNotFound))
    }

    fn get_training_or_panic(env: &Env, training_id: BytesN<32>) -> TrainingRecord {
        env.storage()
            .instance()
            .get(&PrivacyKey::TrainingRecord(training_id))
            .unwrap_or_else(|| panic_with_error!(env, PrivacyError::TrainingNotFound))
    }

    fn get_consent_or_panic(env: &Env, consent_id: BytesN<32>) -> ConsentRecord {
        env.storage()
            .instance()
            .get(&PrivacyKey::ConsentRecord(consent_id))
            .unwrap_or_else(|| panic_with_error!(env, PrivacyError::ConsentNotFound))
    }

    fn get_next_dsr_id(env: &Env) -> u32 {
        let current = env
            .storage()
            .instance()
            .get::<_, u32>(&PrivacyKey::NextDsrId)
            .unwrap_or(1);
        env.storage()
            .instance()
            .set(&PrivacyKey::NextDsrId, &(current + 1));
        current
    }

    fn get_next_breach_id(env: &Env) -> u32 {
        let current = env
            .storage()
            .instance()
            .get::<_, u32>(&PrivacyKey::NextBreachId)
            .unwrap_or(1);
        env.storage()
            .instance()
            .set(&PrivacyKey::NextBreachId, &(current + 1));
        current
    }

    fn get_next_dpia_id(env: &Env) -> u32 {
        let current = env
            .storage()
            .instance()
            .get::<_, u32>(&PrivacyKey::NextDpiaId)
            .unwrap_or(1);
        env.storage()
            .instance()
            .set(&PrivacyKey::NextDpiaId, &(current + 1));
        current
    }

    fn get_next_training_id(env: &Env) -> u32 {
        let current = env
            .storage()
            .instance()
            .get::<_, u32>(&PrivacyKey::NextTrainingId)
            .unwrap_or(1);
        env.storage()
            .instance()
            .set(&PrivacyKey::NextTrainingId, &(current + 1));
        current
    }

    fn get_next_consent_id(env: &Env) -> u32 {
        let current = env
            .storage()
            .instance()
            .get::<_, u32>(&PrivacyKey::NextConsentId)
            .unwrap_or(1);
        env.storage()
            .instance()
            .set(&PrivacyKey::NextConsentId, &(current + 1));
        current
    }

    fn sha2_digest(env: &Env, data: &[u8]) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&result);
        arr
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dsr_lifecycle() {
        let env = Env::default();
        let owner = Address::from_array(&env, &[1; 32]);
        let requester = Address::from_array(&env, &[2; 32]);

        PrivacyMetrics::initialize(env.clone(), owner.clone());

        let dsr = PrivacyMetrics::submit_dsr(
            env.clone(),
            requester.clone(),
            0,
            Bytes::new(&env),
        );
        assert_eq!(dsr.dsr_type, 0);
        assert_eq!(dsr.status, Symbol::new(&env, "pending"));

        let ids = PrivacyMetrics::list_dsr_ids(env.clone());
        assert_eq!(ids.len(), 1);

        PrivacyMetrics::resolve_dsr(env.clone(), owner, dsr.id);
        let resolved = PrivacyMetrics::get_dsr(env, dsr.id);
        assert_eq!(resolved.status, Symbol::new(&env, "resolved"));
    }

    #[test]
    fn test_breach_lifecycle() {
        let env = Env::default();
        let owner = Address::from_array(&env, &[1; 32]);
        let reporter = Address::from_array(&env, &[2; 32]);

        PrivacyMetrics::initialize(env.clone(), owner.clone());

        let breach = PrivacyMetrics::report_breach(
            env.clone(),
            reporter,
            2,
            Bytes::new(&env),
            Vec::new(&env),
        );
        assert!(!breach.resolved);

        PrivacyMetrics::resolve_breach(env.clone(), owner, breach.id.clone(), Bytes::new(&env));
        let resolved = PrivacyMetrics::get_breach(env, breach.id);
        assert!(resolved.resolved);
    }

    #[test]
    fn test_dpia_lifecycle() {
        let env = Env::default();
        let owner = Address::from_array(&env, &[1; 32]);
        let assessor = Address::from_array(&env, &[2; 32]);

        PrivacyMetrics::initialize(env.clone(), owner.clone());

        let dpia = PrivacyMetrics::create_dpia(
            env.clone(),
            assessor,
            Bytes::new(&env),
            2,
            Bytes::new(&env),
        );
        assert!(!dpia.completed);

        PrivacyMetrics::complete_dpia(env.clone(), owner, dpia.id);
        let completed = PrivacyMetrics::get_dpia(env, dpia.id);
        assert!(completed.completed);
    }

    #[test]
    fn test_privacy_snapshot() {
        let env = Env::default();
        let owner = Address::from_array(&env, &[1; 32]);

        PrivacyMetrics::initialize(env.clone(), owner.clone());

        let snapshot = PrivacyMetrics::compute_privacy_snapshot(env.clone(), owner);
        assert_eq!(snapshot.total_dsrs, 0);
        assert_eq!(snapshot.total_breaches, 0);
        assert_eq!(snapshot.dpia_completion_rate_bps, 10000);
    }
}
