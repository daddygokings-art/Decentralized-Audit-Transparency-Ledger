//! Synthetic Monitoring and SLA Tracking for Decentralized Audit Ledger
//!
//! Implements synthetic monitoring across critical user journeys:
//! - Event submission & cryptographic hash-chain verification
//! - Event query, filtering, and indexed pagination
//! - Governance operations (proposal lifecycle, voting, quorum)
//! - API health checks & node RPC availability
//! - Continuous uptime calculation and SLA violation detection

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Bytes,
    Env, Symbol, Vec,
};

// ============================================================================
// Data Structures
// ============================================================================

/// Critical user journeys monitored synthetically
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum SyntheticJourneyType {
    EventSubmission = 0,
    EventQuery = 1,
    GovernanceOperations = 2,
    TokenGatingVerify = 3,
    ApiHealthCheck = 4,
    CrossChainBridgeProbe = 5,
}

/// Execution status of a synthetic probe
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum ProbeStatus {
    Success = 0,
    Degraded = 1,
    Failed = 2,
    Timeout = 3,
}

/// Synthetic probe configuration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntheticProbeConfig {
    pub probe_id: u32,
    pub name: Bytes,
    pub journey_type: SyntheticJourneyType,
    pub endpoint_url: Bytes,
    pub interval_seconds: u32,
    pub timeout_ms: u32,
    pub expected_status: u32,
    pub is_active: bool,
}

/// Execution telemetry from a synthetic probe run
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntheticProbeExecution {
    pub probe_id: u32,
    pub execution_id: u64,
    pub journey_type: SyntheticJourneyType,
    pub timestamp: u64,
    pub duration_ms: u32,
    pub status: ProbeStatus,
    pub status_code: u32,
    pub error_code: Option<Symbol>,
}

/// Target SLA requirements per journey
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlaTarget {
    pub journey_type: SyntheticJourneyType,
    pub target_uptime_bps: u32, // Basis points (e.g. 9990 = 99.90%)
    pub max_latency_p95_ms: u32,
    pub max_latency_p99_ms: u32,
    pub eval_window_seconds: u64,
}

/// Evaluated SLA compliance report over a time window
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlaEvaluationReport {
    pub journey_type: SyntheticJourneyType,
    pub window_start: u64,
    pub window_end: u64,
    pub total_probes: u32,
    pub successful_probes: u32,
    pub uptime_bps: u32,
    pub avg_latency_ms: u32,
    pub p95_latency_ms: u32,
    pub is_sla_met: bool,
    pub consecutive_failures: u32,
}

/// Severity classification for synthetic incidents
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum IncidentSeverity {
    Low = 0,
    Medium = 1,
    High = 2,
    Critical = 3,
}

/// Synthetic monitoring incident record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntheticIncident {
    pub incident_id: u32,
    pub journey_type: SyntheticJourneyType,
    pub severity: IncidentSeverity,
    pub started_at: u64,
    pub resolved_at: u64,
    pub summary: Bytes,
    pub is_resolved: bool,
}

// ============================================================================
// Storage Keys
// ============================================================================

#[contracttype]
pub enum SyntheticKey {
    Owner,
    Probe(u32),
    AllProbeIds,
    RecentExecutions(u32), // probe_id -> Vec<SyntheticProbeExecution>
    SlaTarget(u32),        // journey_type as u32 -> SlaTarget
    ConsecutiveFailures(u32), // journey_type as u32 -> u32
    Incident(u32),
    AllIncidentIds,
    ReporterAllowlist(Address),
}

// ============================================================================
// Contract Errors
// ============================================================================

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum SyntheticError {
    Unauthorized = 1,
    ProbeNotFound = 2,
    DuplicateProbe = 3,
    InvalidInterval = 4,
    InvalidSlaTarget = 5,
    IncidentNotFound = 6,
    IncidentAlreadyResolved = 7,
    ReporterNotAuthorized = 8,
}

// ============================================================================
// Contract Implementation
// ============================================================================

#[contract]
pub struct SyntheticMonitoringEngine;

#[contractimpl]
impl SyntheticMonitoringEngine {
    /// Initialize synthetic monitoring engine with admin/owner
    pub fn initialize(env: Env, owner: Address) {
        owner.require_auth();

        if env.storage().instance().has(&SyntheticKey::Owner) {
            panic_with_error!(&env, SyntheticError::Unauthorized);
        }

        env.storage().instance().set(&SyntheticKey::Owner, &owner);
        let empty_probes: Vec<u32> = Vec::new(&env);
        env.storage().instance().set(&SyntheticKey::AllProbeIds, &empty_probes);
        let empty_incidents: Vec<u32> = Vec::new(&env);
        env.storage().instance().set(&SyntheticKey::AllIncidentIds, &empty_incidents);

        // Authorize owner as default telemetry reporter
        env.storage().instance().set(&SyntheticKey::ReporterAllowlist(owner.clone()), &true);

        // Set default 99.90% SLA targets across critical journeys
        let default_sla = SlaTarget {
            journey_type: SyntheticJourneyType::EventSubmission,
            target_uptime_bps: 9990, // 99.90%
            max_latency_p95_ms: 600,
            max_latency_p99_ms: 1500,
            eval_window_seconds: 86400,
        };
        env.storage().instance().set(
            &SyntheticKey::SlaTarget(SyntheticJourneyType::EventSubmission as u32),
            &default_sla,
        );

        let query_sla = SlaTarget {
            journey_type: SyntheticJourneyType::EventQuery,
            target_uptime_bps: 9995, // 99.95%
            max_latency_p95_ms: 250,
            max_latency_p99_ms: 800,
            eval_window_seconds: 86400,
        };
        env.storage().instance().set(
            &SyntheticKey::SlaTarget(SyntheticJourneyType::EventQuery as u32),
            &query_sla,
        );

        let gov_sla = SlaTarget {
            journey_type: SyntheticJourneyType::GovernanceOperations,
            target_uptime_bps: 9990,
            max_latency_p95_ms: 1000,
            max_latency_p99_ms: 3000,
            eval_window_seconds: 86400,
        };
        env.storage().instance().set(
            &SyntheticKey::SlaTarget(SyntheticJourneyType::GovernanceOperations as u32),
            &gov_sla,
        );

        let api_sla = SlaTarget {
            journey_type: SyntheticJourneyType::ApiHealthCheck,
            target_uptime_bps: 9999, // 99.99%
            max_latency_p95_ms: 150,
            max_latency_p99_ms: 400,
            eval_window_seconds: 86400,
        };
        env.storage().instance().set(
            &SyntheticKey::SlaTarget(SyntheticJourneyType::ApiHealthCheck as u32),
            &api_sla,
        );
    }

    /// Authorize a telemetry reporter address (e.g. synthetic agent runner)
    pub fn set_reporter_status(env: Env, admin: Address, reporter: Address, allowed: bool) {
        admin.require_auth();
        Self::require_owner(&env, &admin);

        env.storage()
            .instance()
            .set(&SyntheticKey::ReporterAllowlist(reporter), &allowed);
    }

    /// Register a new synthetic monitoring probe
    pub fn register_probe(env: Env, admin: Address, config: SyntheticProbeConfig) {
        admin.require_auth();
        Self::require_owner(&env, &admin);

        if config.interval_seconds == 0 || config.timeout_ms == 0 {
            panic_with_error!(&env, SyntheticError::InvalidInterval);
        }

        let mut probe_ids: Vec<u32> = env
            .storage()
            .instance()
            .get(&SyntheticKey::AllProbeIds)
            .unwrap_or_else(|| Vec::new(&env));

        for id in probe_ids.iter() {
            if id == config.probe_id {
                panic_with_error!(&env, SyntheticError::DuplicateProbe);
            }
        }

        probe_ids.push_back(config.probe_id);
        env.storage().instance().set(&SyntheticKey::AllProbeIds, &probe_ids);
        env.storage().instance().set(&SyntheticKey::Probe(config.probe_id), &config);

        env.events().publish(
            (Symbol::new(&env, "synth_probe_created"), config.probe_id),
            (config.journey_type, config.interval_seconds),
        );
    }

    /// Update an existing synthetic probe configuration
    pub fn update_probe(env: Env, admin: Address, config: SyntheticProbeConfig) {
        admin.require_auth();
        Self::require_owner(&env, &admin);

        if !env.storage().instance().has(&SyntheticKey::Probe(config.probe_id)) {
            panic_with_error!(&env, SyntheticError::ProbeNotFound);
        }

        env.storage().instance().set(&SyntheticKey::Probe(config.probe_id), &config);

        env.events().publish(
            (Symbol::new(&env, "synth_probe_updated"), config.probe_id),
            (config.is_active, config.timeout_ms),
        );
    }

    /// Get probe configuration by ID
    pub fn get_probe(env: Env, probe_id: u32) -> SyntheticProbeConfig {
        env.storage()
            .instance()
            .get(&SyntheticKey::Probe(probe_id))
            .unwrap_or_else(|| panic_with_error!(&env, SyntheticError::ProbeNotFound))
    }

    /// Get all registered probe IDs
    pub fn get_all_probe_ids(env: Env) -> Vec<u32> {
        env.storage()
            .instance()
            .get(&SyntheticKey::AllProbeIds)
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Record a synthetic probe execution telemetry result
    pub fn record_probe_execution(
        env: Env,
        reporter: Address,
        exec: SyntheticProbeExecution,
    ) {
        reporter.require_auth();
        let is_allowed = env
            .storage()
            .instance()
            .get(&SyntheticKey::ReporterAllowlist(reporter.clone()))
            .unwrap_or(false);

        if !is_allowed {
            panic_with_error!(&env, SyntheticError::ReporterNotAuthorized);
        }

        if !env.storage().instance().has(&SyntheticKey::Probe(exec.probe_id)) {
            panic_with_error!(&env, SyntheticError::ProbeNotFound);
        }

        let mut executions: Vec<SyntheticProbeExecution> = env
            .storage()
            .instance()
            .get(&SyntheticKey::RecentExecutions(exec.probe_id))
            .unwrap_or_else(|| Vec::new(&env));

        // Maintain bounded ring buffer of latest 50 executions per probe to manage storage
        if executions.len() >= 50 {
            let mut trimmed: Vec<SyntheticProbeExecution> = Vec::new(&env);
            let mut idx = 0u32;
            for e in executions.iter() {
                if idx > 0 {
                    trimmed.push_back(e);
                }
                idx += 1;
            }
            executions = trimmed;
        }

        executions.push_back(exec.clone());
        env.storage()
            .instance()
            .set(&SyntheticKey::RecentExecutions(exec.probe_id), &executions);

        // Update consecutive failures for journey
        let journey_key = exec.journey_type as u32;
        let mut consecutive_failures: u32 = env
            .storage()
            .instance()
            .get(&SyntheticKey::ConsecutiveFailures(journey_key))
            .unwrap_or(0);

        if exec.status == ProbeStatus::Success {
            consecutive_failures = 0;
        } else {
            consecutive_failures += 1;
        }
        env.storage().instance().set(
            &SyntheticKey::ConsecutiveFailures(journey_key),
            &consecutive_failures,
        );

        env.events().publish(
            (Symbol::new(&env, "synth_exec_recorded"), exec.probe_id, exec.journey_type),
            (exec.duration_ms, exec.status, consecutive_failures),
        );
    }

    /// Set SLA target parameters for a user journey
    pub fn set_sla_target(env: Env, admin: Address, target: SlaTarget) {
        admin.require_auth();
        Self::require_owner(&env, &admin);

        if target.target_uptime_bps > 10000 || target.eval_window_seconds == 0 {
            panic_with_error!(&env, SyntheticError::InvalidSlaTarget);
        }

        env.storage().instance().set(
            &SyntheticKey::SlaTarget(target.journey_type as u32),
            &target,
        );
    }

    /// Retrieve SLA target configuration for a journey
    pub fn get_sla_target(env: Env, journey_type: SyntheticJourneyType) -> SlaTarget {
        env.storage()
            .instance()
            .get(&SyntheticKey::SlaTarget(journey_type as u32))
            .unwrap_or_else(|| SlaTarget {
                journey_type,
                target_uptime_bps: 9990,
                max_latency_p95_ms: 1000,
                max_latency_p99_ms: 2500,
                eval_window_seconds: 86400,
            })
    }

    /// Evaluate SLA report and health metrics for a user journey
    pub fn evaluate_journey_sla(env: Env, journey_type: SyntheticJourneyType) -> SlaEvaluationReport {
        let probe_ids = Self::get_all_probe_ids(env.clone());
        let target = Self::get_sla_target(env.clone(), journey_type);
        let now = env.ledger().timestamp();
        let window_start = if now >= target.eval_window_seconds {
            now - target.eval_window_seconds
        } else {
            0
        };

        let mut total_probes = 0u32;
        let mut successful_probes = 0u32;
        let mut total_latency = 0u64;

        for p_id in probe_ids.iter() {
            if let Some(probe) = env.storage().instance().get::<_, SyntheticProbeConfig>(&SyntheticKey::Probe(p_id)) {
                if probe.journey_type == journey_type && probe.is_active {
                    if let Some(execs) = env.storage().instance().get::<_, Vec<SyntheticProbeExecution>>(&SyntheticKey::RecentExecutions(p_id)) {
                        for exec in execs.iter() {
                            if exec.timestamp >= window_start {
                                total_probes += 1;
                                total_latency += exec.duration_ms as u64;
                                if exec.status == ProbeStatus::Success {
                                    successful_probes += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        let uptime_bps = if total_probes > 0 {
            ((successful_probes as u64 * 10000) / (total_probes as u64)) as u32
        } else {
            10000 // Default 100% if no probes yet
        };

        let avg_latency_ms = if total_probes > 0 {
            (total_latency / (total_probes as u64)) as u32
        } else {
            0
        };

        // Approximate p95 as 1.4x average under typical distribution
        let p95_latency_ms = (avg_latency_ms * 14) / 10;

        let consecutive_failures: u32 = env
            .storage()
            .instance()
            .get(&SyntheticKey::ConsecutiveFailures(journey_type as u32))
            .unwrap_or(0);

        let is_sla_met = uptime_bps >= target.target_uptime_bps && p95_latency_ms <= target.max_latency_p95_ms;

        let report = SlaEvaluationReport {
            journey_type,
            window_start,
            window_end: now,
            total_probes,
            successful_probes,
            uptime_bps,
            avg_latency_ms,
            p95_latency_ms,
            is_sla_met,
            consecutive_failures,
        };

        env.events().publish(
            (Symbol::new(&env, "sla_evaluated"), journey_type),
            (uptime_bps, is_sla_met),
        );

        if !is_sla_met {
            env.events().publish(
                (Symbol::new(&env, "sla_breached"), journey_type),
                (uptime_bps, target.target_uptime_bps),
            );
        }

        report
    }

    /// Open an incident triggered by synthetic failure or SLA breach
    pub fn open_synthetic_incident(
        env: Env,
        admin: Address,
        incident_id: u32,
        journey_type: SyntheticJourneyType,
        severity: IncidentSeverity,
        summary: Bytes,
    ) {
        admin.require_auth();
        Self::require_owner(&env, &admin);

        let now = env.ledger().timestamp();
        let incident = SyntheticIncident {
            incident_id,
            journey_type,
            severity,
            started_at: now,
            resolved_at: 0,
            summary,
            is_resolved: false,
        };

        let mut incidents: Vec<u32> = env
            .storage()
            .instance()
            .get(&SyntheticKey::AllIncidentIds)
            .unwrap_or_else(|| Vec::new(&env));
        incidents.push_back(incident_id);
        env.storage().instance().set(&SyntheticKey::AllIncidentIds, &incidents);
        env.storage().instance().set(&SyntheticKey::Incident(incident_id), &incident);

        env.events().publish(
            (Symbol::new(&env, "synth_incident_opened"), incident_id),
            (journey_type, severity),
        );
    }

    /// Resolve an active synthetic incident
    pub fn resolve_synthetic_incident(env: Env, admin: Address, incident_id: u32) {
        admin.require_auth();
        Self::require_owner(&env, &admin);

        let mut incident: SyntheticIncident = env
            .storage()
            .instance()
            .get(&SyntheticKey::Incident(incident_id))
            .unwrap_or_else(|| panic_with_error!(&env, SyntheticError::IncidentNotFound));

        if incident.is_resolved {
            panic_with_error!(&env, SyntheticError::IncidentAlreadyResolved);
        }

        let now = env.ledger().timestamp();
        incident.is_resolved = true;
        incident.resolved_at = now;
        env.storage().instance().set(&SyntheticKey::Incident(incident_id), &incident);

        env.events().publish(
            (Symbol::new(&env, "synth_incident_resolved"), incident_id),
            now,
        );
    }

    /// Retrieve synthetic incident details
    pub fn get_synthetic_incident(env: Env, incident_id: u32) -> SyntheticIncident {
        env.storage()
            .instance()
            .get(&SyntheticKey::Incident(incident_id))
            .unwrap_or_else(|| panic_with_error!(&env, SyntheticError::IncidentNotFound))
    }

    /// Retrieve full system health overview across all monitored journeys
    pub fn get_system_uptime_overview(env: Env) -> Vec<SlaEvaluationReport> {
        let mut reports: Vec<SlaEvaluationReport> = Vec::new(&env);
        reports.push_back(Self::evaluate_journey_sla(env.clone(), SyntheticJourneyType::EventSubmission));
        reports.push_back(Self::evaluate_journey_sla(env.clone(), SyntheticJourneyType::EventQuery));
        reports.push_back(Self::evaluate_journey_sla(env.clone(), SyntheticJourneyType::GovernanceOperations));
        reports.push_back(Self::evaluate_journey_sla(env.clone(), SyntheticJourneyType::ApiHealthCheck));
        reports
    }

    /// Internal helper to check owner authority
    fn require_owner(env: &Env, admin: &Address) {
        let owner: Address = env
            .storage()
            .instance()
            .get(&SyntheticKey::Owner)
            .unwrap_or_else(|| panic_with_error!(env, SyntheticError::Unauthorized));
        if *admin != owner {
            panic_with_error!(env, SyntheticError::Unauthorized);
        }
    }
}
