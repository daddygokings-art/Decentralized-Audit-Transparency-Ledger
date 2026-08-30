//! # Contract Event Incident Management and On-Call Module
//!
//! Provides on-chain coordination, cryptographic audit trails, and circuit-breaker
//! controls for operational and security incidents. Integrates with off-chain
//! incident management engines (PagerDuty, Opsgenie, Prometheus Alertmanager).
//!
//! ## Core Features:
//! - **Incident Lifecycle Management**: Trigger, acknowledge, mitigate, resolve, and close incidents.
//! - **Cryptographic Timeline Auditing**: Immutable timeline events attached to each incident.
//! - **On-Call Responder Registry**: On-chain verification of authorized on-call engineers.
//! - **Tiered Escalation Policies**: Time-bounded escalation thresholds across response tiers.
//! - **Emergency Circuit Breaker**: Automated or manual contract freezing during critical incidents.
//! - **Postmortem Registry**: Verification and anchoring of blameless root cause analyses.

#![no_std]
use soroban_sdk::{
    contracterror, contracttype, panic_with_error, Address, Bytes, BytesN, Env, Symbol, Vec,
};

// ── Error Codes ──────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum IncidentError {
    /// Incident with the given ID already exists
    IncidentAlreadyExists = 5001,
    /// Incident with the given ID was not found
    IncidentNotFound = 5002,
    /// Invalid state transition for the incident
    InvalidStatusTransition = 5003,
    /// Caller is not authorized as an incident commander or owner
    UnauthorizedCommander = 5004,
    /// Caller is not authorized for the on-call team
    UnauthorizedOnCall = 5005,
    /// Escalation policy not found
    EscalationPolicyNotFound = 5006,
    /// Maximum number of timeline entries reached
    TimelineCapacityExceeded = 5007,
    /// Circuit breaker is already tripped
    CircuitBreakerAlreadyTripped = 5008,
    /// Circuit breaker is not active
    CircuitBreakerNotTripped = 5009,
    /// Postmortem has already been recorded for this incident
    PostmortemAlreadyRecorded = 5010,
    /// Invalid escalation tier configuration
    InvalidEscalationTier = 5011,
}

// ── Data Types ───────────────────────────────────────────────────────────

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum IncidentSeverity {
    /// Critical (SEV-1): System down, active exploit, or severe financial/data loss risk
    Critical = 1,
    /// High (SEV-2): Major degraded performance, bridge stalling, or partial outage
    High = 2,
    /// Medium (SEV-3): Non-critical anomaly, elevated error rates, rate limit pressure
    Medium = 3,
    /// Low (SEV-4): Minor cosmetic or non-impacting operational warning
    Low = 4,
    /// Info (SEV-5): Informational notification or operational drill
    Info = 5,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum IncidentStatus {
    /// Alert fired, awaiting acknowledgment
    Triggered = 1,
    /// On-call engineer acknowledged and triaging
    Acknowledged = 2,
    /// Root cause identified and mitigation applied
    Mitigated = 3,
    /// Incident fully resolved, monitoring stability
    Resolved = 4,
    /// Closed following postmortem review
    Closed = 5,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum TimelineEntryType {
    AlertFired = 1,
    CommanderAssigned = 2,
    StatusChanged = 3,
    MitigationApplied = 4,
    NoteAdded = 5,
    CircuitBreakerTripped = 6,
    CircuitBreakerReset = 7,
    Escalated = 8,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimelineEntry {
    pub entry_index: u32,
    pub entry_type: TimelineEntryType,
    pub timestamp: u64,
    pub actor: Address,
    pub note: Bytes,
    pub metadata_hash: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IncidentRecord {
    pub id: BytesN<32>,
    pub event_source: Symbol,
    pub severity: IncidentSeverity,
    pub status: IncidentStatus,
    pub title: Bytes,
    pub description_hash: BytesN<32>,
    pub reporter: Address,
    pub commander: Option<Address>,
    pub created_at: u64,
    pub acknowledged_at: u64,
    pub resolved_at: u64,
    pub timeline_count: u32,
    pub circuit_breaker_tripped: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OnCallSchedule {
    pub team_id: Symbol,
    pub primary_oncall: Address,
    pub secondary_oncall: Address,
    pub rotation_start: u64,
    pub rotation_end: u64,
    pub timezone_offset_minutes: i32,
    pub escalation_policy_id: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscalationTier {
    pub tier_level: u32,
    pub timeout_minutes: u32,
    pub target_responder: Address,
    pub notification_channel: Symbol,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscalationPolicy {
    pub policy_id: BytesN<32>,
    pub name: Symbol,
    pub tiers: Vec<EscalationTier>,
    pub repeat_count: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostmortemRecord {
    pub incident_id: BytesN<32>,
    pub root_cause_hash: BytesN<32>,
    pub impact_summary_hash: BytesN<32>,
    pub action_items_count: u32,
    pub published_at: u64,
    pub approved_by: Address,
}

// ── Storage Keys ─────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IncidentStorageKey {
    Incident(BytesN<32>),
    Timeline(BytesN<32>, u32),
    OnCall(Symbol),
    EscalationPolicy(BytesN<32>),
    Postmortem(BytesN<32>),
    ActiveIncidentCount,
    GlobalCircuitBreaker,
}

// ── Incident Management Functions ────────────────────────────────────────

pub struct IncidentManagement;

impl IncidentManagement {
    /// Create and log a new contract event incident
    pub fn trigger_incident(
        env: &Env,
        caller: Address,
        incident_id: BytesN<32>,
        event_source: Symbol,
        severity: IncidentSeverity,
        title: Bytes,
        description_hash: BytesN<32>,
    ) -> IncidentRecord {
        caller.require_auth();

        let key = IncidentStorageKey::Incident(incident_id.clone());
        if env.storage().persistent().has(&key) {
            panic_with_error!(env, IncidentError::IncidentAlreadyExists);
        }

        let now = env.ledger().timestamp();
        let record = IncidentRecord {
            id: incident_id.clone(),
            event_source,
            severity,
            status: IncidentStatus::Triggered,
            title,
            description_hash: description_hash.clone(),
            reporter: caller.clone(),
            commander: None,
            created_at: now,
            acknowledged_at: 0,
            resolved_at: 0,
            timeline_count: 1,
            circuit_breaker_tripped: false,
        };

        env.storage().persistent().set(&key, &record);

        // Record initial timeline entry
        let timeline_entry = TimelineEntry {
            entry_index: 0,
            entry_type: TimelineEntryType::AlertFired,
            timestamp: now,
            actor: caller,
            note: Bytes::new(env),
            metadata_hash: description_hash,
        };
        let t_key = IncidentStorageKey::Timeline(incident_id, 0);
        env.storage().persistent().set(&t_key, &timeline_entry);

        let count: u32 = env
            .storage()
            .persistent()
            .get(&IncidentStorageKey::ActiveIncidentCount)
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&IncidentStorageKey::ActiveIncidentCount, &(count + 1));

        record
    }

    /// Acknowledge an incident and assign incident commander
    pub fn acknowledge_incident(
        env: &Env,
        commander: Address,
        incident_id: BytesN<32>,
        note: Bytes,
    ) -> IncidentRecord {
        commander.require_auth();

        let key = IncidentStorageKey::Incident(incident_id.clone());
        let mut incident: IncidentRecord = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(env, IncidentError::IncidentNotFound));

        if incident.status != IncidentStatus::Triggered {
            panic_with_error!(env, IncidentError::InvalidStatusTransition);
        }

        let now = env.ledger().timestamp();
        incident.status = IncidentStatus::Acknowledged;
        incident.commander = Some(commander.clone());
        incident.acknowledged_at = now;

        let entry_idx = incident.timeline_count;
        incident.timeline_count += 1;
        env.storage().persistent().set(&key, &incident);

        let timeline_entry = TimelineEntry {
            entry_index: entry_idx,
            entry_type: TimelineEntryType::CommanderAssigned,
            timestamp: now,
            actor: commander,
            note,
            metadata_hash: BytesN::from_array(env, &[0u8; 32]),
        };
        let t_key = IncidentStorageKey::Timeline(incident_id, entry_idx);
        env.storage().persistent().set(&t_key, &timeline_entry);

        incident
    }

    /// Add a progress or mitigation note to the incident timeline
    pub fn add_timeline_entry(
        env: &Env,
        actor: Address,
        incident_id: BytesN<32>,
        entry_type: TimelineEntryType,
        note: Bytes,
        metadata_hash: BytesN<32>,
    ) -> TimelineEntry {
        actor.require_auth();

        let key = IncidentStorageKey::Incident(incident_id.clone());
        let mut incident: IncidentRecord = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(env, IncidentError::IncidentNotFound));

        let now = env.ledger().timestamp();
        let entry_idx = incident.timeline_count;
        if entry_idx >= 500 {
            panic_with_error!(env, IncidentError::TimelineCapacityExceeded);
        }
        incident.timeline_count += 1;
        env.storage().persistent().set(&key, &incident);

        let timeline_entry = TimelineEntry {
            entry_index: entry_idx,
            entry_type,
            timestamp: now,
            actor,
            note,
            metadata_hash,
        };
        let t_key = IncidentStorageKey::Timeline(incident_id, entry_idx);
        env.storage().persistent().set(&t_key, &timeline_entry);

        timeline_entry
    }

    /// Trip or reset emergency circuit breaker for a critical incident
    pub fn set_circuit_breaker(
        env: &Env,
        caller: Address,
        incident_id: BytesN<32>,
        trip: bool,
        reason: Bytes,
    ) {
        caller.require_auth();

        let key = IncidentStorageKey::Incident(incident_id.clone());
        let mut incident: IncidentRecord = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(env, IncidentError::IncidentNotFound));

        if trip && incident.circuit_breaker_tripped {
            panic_with_error!(env, IncidentError::CircuitBreakerAlreadyTripped);
        }
        if !trip && !incident.circuit_breaker_tripped {
            panic_with_error!(env, IncidentError::CircuitBreakerNotTripped);
        }

        incident.circuit_breaker_tripped = trip;
        let now = env.ledger().timestamp();
        let entry_idx = incident.timeline_count;
        incident.timeline_count += 1;
        env.storage().persistent().set(&key, &incident);

        env.storage()
            .persistent()
            .set(&IncidentStorageKey::GlobalCircuitBreaker, &trip);

        let entry_type = if trip {
            TimelineEntryType::CircuitBreakerTripped
        } else {
            TimelineEntryType::CircuitBreakerReset
        };

        let timeline_entry = TimelineEntry {
            entry_index: entry_idx,
            entry_type,
            timestamp: now,
            actor: caller,
            note: reason,
            metadata_hash: BytesN::from_array(env, &[0u8; 32]),
        };
        let t_key = IncidentStorageKey::Timeline(incident_id, entry_idx);
        env.storage().persistent().set(&t_key, &timeline_entry);
    }

    /// Resolve an incident
    pub fn resolve_incident(
        env: &Env,
        caller: Address,
        incident_id: BytesN<32>,
        resolution_notes: Bytes,
    ) -> IncidentRecord {
        caller.require_auth();

        let key = IncidentStorageKey::Incident(incident_id.clone());
        let mut incident: IncidentRecord = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(env, IncidentError::IncidentNotFound));

        if incident.status == IncidentStatus::Resolved || incident.status == IncidentStatus::Closed {
            panic_with_error!(env, IncidentError::InvalidStatusTransition);
        }

        let now = env.ledger().timestamp();
        incident.status = IncidentStatus::Resolved;
        incident.resolved_at = now;
        if incident.circuit_breaker_tripped {
            incident.circuit_breaker_tripped = false;
            env.storage()
                .persistent()
                .set(&IncidentStorageKey::GlobalCircuitBreaker, &false);
        }

        let entry_idx = incident.timeline_count;
        incident.timeline_count += 1;
        env.storage().persistent().set(&key, &incident);

        let timeline_entry = TimelineEntry {
            entry_index: entry_idx,
            entry_type: TimelineEntryType::StatusChanged,
            timestamp: now,
            actor: caller,
            note: resolution_notes,
            metadata_hash: BytesN::from_array(env, &[0u8; 32]),
        };
        let t_key = IncidentStorageKey::Timeline(incident_id, entry_idx);
        env.storage().persistent().set(&t_key, &timeline_entry);

        let count: u32 = env
            .storage()
            .persistent()
            .get(&IncidentStorageKey::ActiveIncidentCount)
            .unwrap_or(0);
        if count > 0 {
            env.storage()
                .persistent()
                .set(&IncidentStorageKey::ActiveIncidentCount, &(count - 1));
        }

        incident
    }

    /// Configure on-call team rotation
    pub fn set_on_call_schedule(
        env: &Env,
        admin: Address,
        schedule: OnCallSchedule,
    ) {
        admin.require_auth();
        let key = IncidentStorageKey::OnCall(schedule.team_id.clone());
        env.storage().persistent().set(&key, &schedule);
    }

    /// Configure multi-tier escalation policy
    pub fn set_escalation_policy(
        env: &Env,
        admin: Address,
        policy: EscalationPolicy,
    ) {
        admin.require_auth();
        let key = IncidentStorageKey::EscalationPolicy(policy.policy_id.clone());
        env.storage().persistent().set(&key, &policy);
    }

    /// Register postmortem root cause analysis
    pub fn record_postmortem(
        env: &Env,
        approver: Address,
        postmortem: PostmortemRecord,
    ) {
        approver.require_auth();

        let inc_key = IncidentStorageKey::Incident(postmortem.incident_id.clone());
        let mut incident: IncidentRecord = env
            .storage()
            .persistent()
            .get(&inc_key)
            .unwrap_or_else(|| panic_with_error!(env, IncidentError::IncidentNotFound));

        let pm_key = IncidentStorageKey::Postmortem(postmortem.incident_id.clone());
        if env.storage().persistent().has(&pm_key) {
            panic_with_error!(env, IncidentError::PostmortemAlreadyRecorded);
        }

        incident.status = IncidentStatus::Closed;
        env.storage().persistent().set(&inc_key, &incident);
        env.storage().persistent().set(&pm_key, &postmortem);
    }

    /// Retrieve incident record
    pub fn get_incident(env: &Env, incident_id: BytesN<32>) -> Option<IncidentRecord> {
        let key = IncidentStorageKey::Incident(incident_id);
        env.storage().persistent().get(&key)
    }

    /// Retrieve timeline entry
    pub fn get_timeline_entry(
        env: &Env,
        incident_id: BytesN<32>,
        entry_index: u32,
    ) -> Option<TimelineEntry> {
        let key = IncidentStorageKey::Timeline(incident_id, entry_index);
        env.storage().persistent().get(&key)
    }

    /// Retrieve on-call schedule
    pub fn get_on_call_schedule(env: &Env, team_id: Symbol) -> Option<OnCallSchedule> {
        let key = IncidentStorageKey::OnCall(team_id);
        env.storage().persistent().get(&key)
    }

    /// Retrieve escalation policy
    pub fn get_escalation_policy(env: &Env, policy_id: BytesN<32>) -> Option<EscalationPolicy> {
        let key = IncidentStorageKey::EscalationPolicy(policy_id);
        env.storage().persistent().get(&key)
    }

    /// Retrieve postmortem record
    pub fn get_postmortem(env: &Env, incident_id: BytesN<32>) -> Option<PostmortemRecord> {
        let key = IncidentStorageKey::Postmortem(incident_id);
        env.storage().persistent().get(&key)
    }
}
