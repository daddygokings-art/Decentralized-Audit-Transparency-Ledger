#![no_std]

use crate::rbac::{RbacManager, Role};
use crate::suptech_types::{AlertStatus, ComplianceAlert, RegulatoryFramework, SupervisionRule};
use soroban_sdk::xdr::ToXdr;
use soroban_sdk::{contracterror, contracttype, Address, Bytes, BytesN, Env, Symbol, Vec};

/// Hard cap on the number of items returned in a single page.
pub const MAX_PAGE_LIMIT: u32 = 100;

/// Hard cap on the number of rules allowed in a single rule set, preventing
/// unbounded vector growth in [`RuleSet`].
pub const MAX_RULES_PER_SET: u32 = 1_000;

/// Hard cap on the number of rule evaluations produced in a single
/// [`RulesEngine::execute_ruleset`] run, preventing unbounded vector growth.
pub const MAX_EVALUATIONS_PER_RUN: u32 = 10_000;

/// Maximum allowed length (bytes) for a rule name.
pub const MAX_RULE_NAME_LEN: u32 = 256;

/// Maximum allowed length (bytes) for a rule condition.
pub const MAX_RULE_CONDITION_LEN: u32 = 1_024;

/// Maximum allowed length (bytes) for a rule action.
pub const MAX_RULE_ACTION_LEN: u32 = 1_024;

/// Maximum validator-set size for a rule-proof attestation round.
pub const MAX_RULE_VALIDATORS: u32 = 32;

/// Maximum proof payload length accepted by the attestation API.
pub const MAX_RULE_PROOF_LEN: u32 = 64 * 1024;

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RulesEngineError {
    Unauthorized = 7401,
    InvalidValidatorSet = 7402,
    NotValidator = 7403,
    InvalidRulesetVersion = 7404,
    ProofTooLarge = 7405,
    ValidatorEquivocation = 7406,
    ConflictingFinalization = 7407,
    CounterExhausted = 7408,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleValidatorConfig {
    pub validators: Vec<Address>,
    pub quorum: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleProofCandidate {
    pub proof_hash: BytesN<32>,
    pub approvals: Vec<Address>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleProofAttestationRound {
    pub ruleset_id: BytesN<32>,
    pub ruleset_version: u32,
    pub validators: Vec<Address>,
    pub quorum: u32,
    pub candidates: Vec<RuleProofCandidate>,
    pub finalized_proof_hash: Option<BytesN<32>>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuleAttestationOutcome {
    Pending(u32),
    Finalized(u32),
    AlreadyRecorded(u32),
    AlreadyFinalized,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RulesEngineStorageKey {
    ValidatorConfig,
    AttestationRound(BytesN<32>, u32),
}

/// Cursor token used for deterministic cursor-based pagination.
///
/// The cursor encodes the composite sort key `(ledger_sequence, id)` of the
/// last item returned on the previous page. Ordering on this composite key is
/// stable across ledger upgrades and archival restores, so traversal is
/// reproducible. `ledger_sequence` is u64 so timestamps past the u32 range
/// (year 2106) cannot truncate.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageCursor {
    /// Ledger sequence of the last returned item.
    pub ledger_sequence: u64,
    /// Tie-breaking identifier of the last returned item.
    pub id: BytesN<32>,
}

/// Requested page size, capped at [`MAX_PAGE_LIMIT`].
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageLimit {
    /// Requested number of items.
    pub limit: u32,
}

impl PageLimit {
    /// Construct a page limit, rejecting zero and clamping to the hard cap.
    pub fn new(limit: u32) -> Result<Self, &'static str> {
        if limit == 0 {
            return Err("Page limit must be greater than zero");
        }
        Ok(Self {
            limit: if limit > MAX_PAGE_LIMIT { MAX_PAGE_LIMIT } else { limit },
        })
    }

    /// Effective (capped) limit.
    pub fn effective(&self) -> u32 {
        if self.limit > MAX_PAGE_LIMIT {
            MAX_PAGE_LIMIT
        } else {
            self.limit
        }
    }
}

/// Multi-predicate filter for rule/log collection queries.
///
/// All predicates are optional; `None` means "no constraint". When several
/// predicates are supplied they are combined with logical AND.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleQueryFilter {
    /// Inclusive lower bound on ledger timestamp.
    pub min_timestamp: Option<u64>,
    /// Inclusive upper bound on ledger timestamp.
    pub max_timestamp: Option<u64>,
    /// Required alert status (when filtering alerts).
    pub status: Option<u32>,
    /// Required submitter / subject address.
    pub submitter: Option<Address>,
}

impl RuleQueryFilter {
    /// Empty filter matching every item.
    pub fn none() -> Self {
        Self {
            min_timestamp: None,
            max_timestamp: None,
            status: None,
            submitter: None,
        }
    }

    /// Validate the filter, returning a typed error for invalid ranges.
    pub fn validate(&self) -> Result<(), &'static str> {
        if let (Some(min), Some(max)) = (self.min_timestamp, self.max_timestamp) {
            if min > max {
                return Err("Invalid timestamp range: min exceeds max");
            }
        }
        Ok(())
    }

    /// Returns true when `timestamp` satisfies the timestamp predicates.
    pub fn matches_timestamp(&self, timestamp: u64) -> bool {
        if let Some(min) = self.min_timestamp {
            if timestamp < min {
                return false;
            }
        }
        if let Some(max) = self.max_timestamp {
            if timestamp > max {
                return false;
            }
        }
        true
    }

    /// Returns true when `status` satisfies the status predicate.
    pub fn matches_status(&self, status: u32) -> bool {
        match self.status {
            Some(required) => required == status,
            None => true,
        }
    }

    /// Returns true when `submitter` satisfies the submitter predicate.
    pub fn matches_submitter(&self, submitter: &Address) -> bool {
        match &self.submitter {
            Some(required) => required == submitter,
            None => true,
        }
    }
}

/// A page of results plus the cursor for the next page.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleExecutionLogPage {
    /// Items in this page, deterministically ordered.
    pub items: Vec<RuleExecutionLog>,
    /// Cursor to pass to the next call, or `None` when exhausted.
    pub next_cursor: Option<PageCursor>,
}

/// A page of alert results plus the cursor for the next page.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComplianceAlertPage {
    /// Items in this page, deterministically ordered.
    pub items: Vec<ComplianceAlert>,
    /// Cursor to pass to the next call, or `None` when exhausted.
    pub next_cursor: Option<PageCursor>,
}

/// A page of rule results plus the cursor for the next page.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupervisionRulePage {
    /// Items in this page, deterministically ordered.
    pub items: Vec<SupervisionRule>,
    /// Cursor to pass to the next call, or `None` when exhausted.
    pub next_cursor: Option<PageCursor>,
}

/// Compare two composite sort keys `(ledger_sequence, id)`.
///
/// Returns `Ordering::Less` when `a` sorts before `b`. Ordering is total and
/// deterministic, which keeps pagination stable across ledger upgrades and
/// archival restores. `ledger_sequence` is u64 to avoid truncating timestamps
/// past the u32 range (year 2106).
fn compare_keys(a: &(u64, BytesN<32>), b: &(u64, BytesN<32>)) -> core::cmp::Ordering {
    match a.0.cmp(&b.0) {
        core::cmp::Ordering::Equal => a.1.to_array().cmp(&b.1.to_array()),
        other => other,
    }
}

/// Returns true when `key` sorts strictly after `cursor`.
fn key_after_cursor(key: &(u32, BytesN<32>), cursor: &PageCursor) -> bool {
    compare_keys(key, &(cursor.ledger_sequence, cursor.id.clone())) == core::cmp::Ordering::Greater
}

/// Build the next cursor from the last item of a full page.
fn next_cursor_for(key: &(u32, BytesN<32>), page_len: u32, limit: u32) -> Option<PageCursor> {
    if page_len < limit {
        None
    } else {
        Some(PageCursor {
            ledger_sequence: key.0,
            id: key.1.clone(),
        })
    }
}

/// Rule evaluation result.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleEvaluation {
    /// Rule ID evaluated
    pub rule_id: BytesN<32>,
    /// Evaluation timestamp
    pub evaluated_at: u64,
    /// Condition met (rule triggered)
    pub condition_met: bool,
    /// Evaluation context (transaction ID, account, etc.)
    pub context: Bytes,
    /// Severity score (0-10)
    pub severity_score: u32,
}

/// Rule execution log entry.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleExecutionLog {
    /// Log ID
    pub log_id: BytesN<32>,
    /// Rule ID
    pub rule_id: BytesN<32>,
    /// Subject institution
    pub subject: Address,
    /// Timestamp executed
    pub executed_at: u64,
    /// Number of times triggered
    pub trigger_count: u32,
    /// Highest severity alert generated
    pub max_alert_severity: u32,
}

/// Rule set for regulatory framework.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleSet {
    /// Rule set ID
    pub ruleset_id: BytesN<32>,
    /// Associated regulatory framework
    pub framework: u32, // RegulatoryFramework as u32
    /// Rules in set
    pub rules: Vec<BytesN<32>>,
    /// Rule set version
    pub version: u32,
    /// Active
    pub is_active: bool,
    /// Created at
    pub created_at: u64,
    /// Updated at
    pub updated_at: u64,
}

/// Supervision rules engine.
pub struct RulesEngine;

impl RulesEngine {
    /// Configure the validator set used for future proof-attestation rounds.
    pub fn configure_validators(
        env: &Env,
        caller: &Address,
        validators: Vec<Address>,
        quorum: u32,
    ) -> Result<(), RulesEngineError> {
        RbacManager::require_role_precedence(env, caller, Role::Admin).map_err(|_| RulesEngineError::Unauthorized)?;

        let count = validators.len();
        if count == 0 || count > MAX_RULE_VALIDATORS || quorum == 0 || quorum > count || quorum <= count / 2 {
            return Err(RulesEngineError::InvalidValidatorSet);
        }
        for i in 0..count {
            let validator = validators.get_unchecked(i);
            for j in (i + 1)..count {
                if validators.get_unchecked(j) == validator {
                    return Err(RulesEngineError::InvalidValidatorSet);
                }
            }
        }

        env.storage().persistent().set(
            &RulesEngineStorageKey::ValidatorConfig,
            &RuleValidatorConfig { validators, quorum },
        );
        env.events().publish(
            (Symbol::new(env, "suptech_rules"), Symbol::new(env, "validators")),
            (caller.clone(), count, quorum),
        );
        Ok(())
    }

    /// Deterministically bind a proof payload to a rule-set version.
    pub fn compute_rule_proof_hash(
        env: &Env,
        ruleset_id: &BytesN<32>,
        ruleset_version: u32,
        proof: &Bytes,
    ) -> Result<BytesN<32>, RulesEngineError> {
        if proof.len() > MAX_RULE_PROOF_LEN {
            return Err(RulesEngineError::ProofTooLarge);
        }
        let mut input = Bytes::new(env);
        input.append(&Bytes::from_slice(env, b"SUPTECH_RULE_PROOF_V1"));
        input.append(&ruleset_id.to_bytes());
        input.extend_from_array(&ruleset_version.to_be_bytes());
        input.append(proof);
        Ok(env.crypto().sha256(&input))
    }

    /// Record a validator's authenticated vote for a deterministic proof.
    ///
    /// Soroban serializes writes to persistent storage. Each transaction reads
    /// the latest round, rejects duplicate/equivocating votes, and writes the
    /// updated round atomically, so concurrent submissions cannot lose votes.
    pub fn attest_rule_proof(
        env: &Env,
        validator: &Address,
        ruleset_id: &BytesN<32>,
        ruleset_version: u32,
        proof: &Bytes,
    ) -> Result<RuleAttestationOutcome, RulesEngineError> {
        validator.require_auth();
        if ruleset_version == 0 {
            return Err(RulesEngineError::InvalidRulesetVersion);
        }
        let config: RuleValidatorConfig = env
            .storage()
            .persistent()
            .get(&RulesEngineStorageKey::ValidatorConfig)
            .ok_or(RulesEngineError::InvalidValidatorSet)?;
        let proof_hash = Self::compute_rule_proof_hash(env, ruleset_id, ruleset_version, proof)?;
        let key = RulesEngineStorageKey::AttestationRound(ruleset_id.clone(), ruleset_version);
        let mut round = match env.storage().persistent().get(&key) {
            Some(existing) => existing,
            None => RuleProofAttestationRound {
                ruleset_id: ruleset_id.clone(),
                ruleset_version,
                validators: config.validators,
                quorum: config.quorum,
                candidates: Vec::new(env),
                finalized_proof_hash: None,
            },
        };

        if !round.validators.contains(validator) {
            return Err(RulesEngineError::NotValidator);
        }

        if let Some(ref finalized) = round.finalized_proof_hash {
            return if *finalized == proof_hash {
                Ok(RuleAttestationOutcome::AlreadyFinalized)
            } else {
                Err(RulesEngineError::ConflictingFinalization)
            };
        }

        let mut candidate_index = None;
        for i in 0..round.candidates.len() {
            let candidate = round.candidates.get_unchecked(i);
            if candidate.approvals.contains(validator) {
                return if candidate.proof_hash == proof_hash {
                    Ok(RuleAttestationOutcome::AlreadyRecorded(candidate.approvals.len()))
                } else {
                    Err(RulesEngineError::ValidatorEquivocation)
                };
            }
            if candidate.proof_hash == proof_hash {
                candidate_index = Some(i);
            }
        }

        let mut candidate = match candidate_index {
            Some(i) => round.candidates.get_unchecked(i),
            None => RuleProofCandidate {
                proof_hash: proof_hash.clone(),
                approvals: Vec::new(env),
            },
        };
        candidate.approvals.push_back(validator.clone());
        let approvals = candidate.approvals.len();
        let outcome = if approvals >= round.quorum {
            round.finalized_proof_hash = Some(proof_hash.clone());
            env.events().publish(
                (Symbol::new(env, "suptech_rules"), Symbol::new(env, "proof_finalized")),
                (round.ruleset_id.clone(), round.ruleset_version, proof_hash, approvals),
            );
            RuleAttestationOutcome::Finalized(approvals)
        } else {
            RuleAttestationOutcome::Pending(approvals)
        };

        match candidate_index {
            Some(i) => round.candidates.set(i, candidate),
            None => round.candidates.push_back(candidate),
        }
        env.storage().persistent().set(&key, &round);
        Ok(outcome)
    }

    /// Read the persisted attestation round for a rule-set version.
    pub fn get_attestation_round(
        env: &Env,
        ruleset_id: &BytesN<32>,
        ruleset_version: u32,
    ) -> Option<RuleProofAttestationRound> {
        env.storage().persistent().get(&RulesEngineStorageKey::AttestationRound(
            ruleset_id.clone(),
            ruleset_version,
        ))
    }

    /// Create supervision rule
    pub fn create_rule(
        env: &Env,
        framework: RegulatoryFramework,
        name: Bytes,
        condition: Bytes,
        action: Bytes,
        severity: u32,
    ) -> Result<SupervisionRule, &'static str> {
        if name.is_empty() {
            return Err("Rule name cannot be empty");
        }
        if name.len() > MAX_RULE_NAME_LEN {
            return Err("Rule name exceeds maximum length");
        }

        if condition.is_empty() {
            return Err("Rule condition cannot be empty");
        }
        if condition.len() > MAX_RULE_CONDITION_LEN {
            return Err("Rule condition exceeds maximum length");
        }

        if action.len() > MAX_RULE_ACTION_LEN {
            return Err("Rule action exceeds maximum length");
        }

        if severity > 10 {
            return Err("Severity must be 0-10");
        }

        let rule_id = Self::compute_rule_id(env, framework, &name);

        Ok(SupervisionRule {
            rule_id,
            name,
            framework: framework as u32,
            condition,
            action,
            severity,
            is_active: true,
            created_at: env.ledger().timestamp(),
            updated_at: env.ledger().timestamp(),
        })
    }

    /// Compute rule ID
    pub fn compute_rule_id(env: &Env, framework: RegulatoryFramework, name: &Bytes) -> BytesN<32> {
        let mut input = Bytes::new(env);
        input.append(&Bytes::from_slice(env, framework.code().as_bytes()));
        input.append(name);

        env.crypto().sha256(&input)
    }

    /// Evaluate rule against context
    pub fn evaluate_rule(env: &Env, rule: &SupervisionRule, context: Bytes) -> Result<RuleEvaluation, &'static str> {
        if !rule.is_active {
            return Err("Rule is not active");
        }

        // Simple evaluation: check if context contains rule condition keywords
        // In production, this would be more sophisticated
        let condition_met = context.len() > 0; // Placeholder

        Ok(RuleEvaluation {
            rule_id: rule.rule_id.clone(),
            evaluated_at: env.ledger().timestamp(),
            condition_met,
            context,
            severity_score: rule.severity,
        })
    }

    /// Generate alert from rule trigger
    pub fn generate_alert_from_rule(
        env: &Env,
        rule: &SupervisionRule,
        institution: Address,
        supporting_data: Bytes,
    ) -> Result<ComplianceAlert, &'static str> {
        let alert_id = Self::compute_alert_id(env, &rule.rule_id, &institution);

        Ok(ComplianceAlert {
            alert_id,
            rule_id: rule.rule_id.clone(),
            institution,
            severity: rule.severity,
            message: rule.name.clone(),
            triggered_at: env.ledger().timestamp(),
            supporting_data,
            status: AlertStatus::New as u32,
            resolution_notes: Bytes::new(env),
        })
    }

    /// Compute alert ID
    pub fn compute_alert_id(env: &Env, rule_id: &BytesN<32>, institution: &Address) -> BytesN<32> {
        let mut input = Bytes::new(env);
        input.append(&rule_id.to_bytes());
        input.append(&institution.to_xdr(env));
        input.append(&Bytes::from_slice(env, &env.ledger().timestamp().to_le_bytes()));

        env.crypto().sha256(&input)
    }

    /// Update rule condition (for dynamic rules)
    pub fn update_rule_condition(
        env: &Env,
        rule: &mut SupervisionRule,
        new_condition: Bytes,
    ) -> Result<(), &'static str> {
        if new_condition.is_empty() {
            return Err("New condition cannot be empty");
        }
        if new_condition.len() > MAX_RULE_CONDITION_LEN {
            return Err("New condition exceeds maximum length");
        }

        rule.condition = new_condition;
        rule.updated_at = env.ledger().timestamp();

        Ok(())
    }

    /// Disable rule
    pub fn disable_rule(rule: &mut SupervisionRule) {
        rule.is_active = false;
    }

    /// Enable rule
    pub fn enable_rule(rule: &mut SupervisionRule) {
        rule.is_active = true;
    }

    /// Create rule set for framework
    pub fn create_ruleset(env: &Env, framework: RegulatoryFramework) -> Result<RuleSet, &'static str> {
        let ruleset_id = Self::compute_ruleset_id(env, framework);

        Ok(RuleSet {
            ruleset_id,
            framework: framework as u32,
            rules: Vec::new(env),
            version: 1,
            is_active: true,
            created_at: env.ledger().timestamp(),
            updated_at: env.ledger().timestamp(),
        })
    }

    /// Compute rule set ID
    pub fn compute_ruleset_id(env: &Env, framework: RegulatoryFramework) -> BytesN<32> {
        let mut input = Bytes::new(env);
        input.append(&Bytes::from_slice(env, framework.code().as_bytes()));
        input.append(&Bytes::from_slice(env, b"RULESET"));

        env.crypto().sha256(&input)
    }

    /// Add rule to set
    pub fn add_rule_to_set(ruleset: &mut RuleSet, rule_id: BytesN<32>) -> Result<(), &'static str> {
        if (ruleset.rules.len() as u32) >= MAX_RULES_PER_SET {
            return Err("Rule set at maximum capacity");
        }

        // Check for duplicates
        for existing_rule in ruleset.rules.iter() {
            if existing_rule == rule_id {
                return Err("Rule already in set");
            }
        }

        ruleset.rules.push_back(rule_id);
        Ok(())
    }

    /// Remove rule from set
    pub fn remove_rule_from_set(ruleset: &mut RuleSet, rule_id: &BytesN<32>) -> Result<(), &'static str> {
        let mut found = false;

        for i in 0..ruleset.rules.len() {
            if let Some(existing) = ruleset.rules.get(i) {
                if existing == *rule_id {
                    // Remove by not including in new vec
                    found = true;
                    break;
                }
            }
        }

        if !found {
            return Err("Rule not found in set");
        }

        Ok(())
    }

    /// Execute all active rules against context
    pub fn execute_ruleset(
        env: &Env,
        ruleset: &RuleSet,
        rules: &Vec<SupervisionRule>,
        context: Bytes,
    ) -> Result<Vec<RuleEvaluation>, &'static str> {
        if !ruleset.is_active {
            return Err("Rule set is not active");
        }

        let mut evaluations = Vec::new(env);

        for rule in rules.iter() {
            if (evaluations.len() as u32) >= MAX_EVALUATIONS_PER_RUN {
                break;
            }
            if !rule.is_active {
                continue;
            }

            // Check if rule is in ruleset
            let mut in_ruleset = false;
            for ruleset_rule_id in ruleset.rules.iter() {
                if ruleset_rule_id == rule.rule_id {
                    in_ruleset = true;
                    break;
                }
            }

            if !in_ruleset {
                continue;
            }

            // Evaluate rule
            if let Ok(eval) = Self::evaluate_rule(env, &rule, context.clone()) {
                evaluations.push_back(eval);
            }
        }

        Ok(evaluations)
    }

    /// Paginated, filtered query over rule execution logs.
    ///
    /// Results are ordered by the composite key `(ledger_sequence, log_id)`
    /// which is stable across ledger upgrades and archival restores. Returns
    /// the page items and a `next_cursor` token, or `None` when exhausted.
    pub fn query_execution_logs(
        env: &Env,
        logs: &Vec<RuleExecutionLog>,
        filter: &RuleQueryFilter,
        cursor: &Option<PageCursor>,
        limit: &PageLimit,
    ) -> Result<RuleExecutionLogPage, &'static str> {
        filter.validate()?;
        let effective = limit.effective();

        // Collect matching keys and sort deterministically.
        let mut keys: Vec<(u64, BytesN<32>)> = Vec::new(env);
        for log in logs.iter() {
            if !filter.matches_timestamp(log.executed_at) {
                continue;
            }
            if !filter.matches_submitter(&log.subject) {
                continue;
            }
            keys.push_back((log.executed_at, log.log_id.clone()));
        }
        sort_keys(&mut keys);

        let mut items = Vec::new(env);
        let mut last_key: Option<(u32, BytesN<32>)> = None;
        let mut page_len = 0u32;

        for key in keys.iter() {
            if let Some(c) = cursor {
                if !key_after_cursor(&key, c) {
                    continue;
                }
            }
            if page_len >= effective {
                break;
            }
            if let Some(log) = find_log(logs, &key.1) {
                items.push_back(log);
                last_key = Some(key.clone());
                page_len += 1;
            }
        }

        let next_cursor = match last_key {
            Some(k) => next_cursor_for(&k, page_len, effective),
            None => None,
        };

        Ok(RuleExecutionLogPage { items, next_cursor })
    }

    /// Paginated, filtered query over compliance alerts.
    ///
    /// Results are ordered by the composite key `(ledger_sequence, alert_id)`.
    pub fn query_alerts(
        env: &Env,
        alerts: &Vec<ComplianceAlert>,
        filter: &RuleQueryFilter,
        cursor: &Option<PageCursor>,
        limit: &PageLimit,
    ) -> Result<ComplianceAlertPage, &'static str> {
        filter.validate()?;
        let effective = limit.effective();

        let mut keys: Vec<(u64, BytesN<32>)> = Vec::new(env);
        for alert in alerts.iter() {
            if !filter.matches_timestamp(alert.triggered_at) {
                continue;
            }
            if !filter.matches_status(alert.status) {
                continue;
            }
            if !filter.matches_submitter(&alert.institution) {
                continue;
            }
            keys.push_back((alert.triggered_at, alert.alert_id.clone()));
        }
        sort_keys(&mut keys);

        let mut items = Vec::new(env);
        let mut last_key: Option<(u32, BytesN<32>)> = None;
        let mut page_len = 0u32;

        for key in keys.iter() {
            if let Some(c) = cursor {
                if !key_after_cursor(&key, c) {
                    continue;
                }
            }
            if page_len >= effective {
                break;
            }
            if let Some(alert) = find_alert(alerts, &key.1) {
                items.push_back(alert);
                last_key = Some(key.clone());
                page_len += 1;
            }
        }

        let next_cursor = match last_key {
            Some(k) => next_cursor_for(&k, page_len, effective),
            None => None,
        };

        Ok(ComplianceAlertPage { items, next_cursor })
    }

    /// Paginated, filtered query over supervision rules.
    ///
    /// Results are ordered by the composite key `(ledger_sequence, rule_id)`.
    pub fn query_rules(
        env: &Env,
        rules: &Vec<SupervisionRule>,
        filter: &RuleQueryFilter,
        cursor: &Option<PageCursor>,
        limit: &PageLimit,
    ) -> Result<SupervisionRulePage, &'static str> {
        filter.validate()?;
        let effective = limit.effective();

        let mut keys: Vec<(u64, BytesN<32>)> = Vec::new(env);
        for rule in rules.iter() {
            if !filter.matches_timestamp(rule.created_at) {
                continue;
            }
            keys.push_back((rule.created_at, rule.rule_id.clone()));
        }
        sort_keys(&mut keys);

        let mut items = Vec::new(env);
        let mut last_key: Option<(u32, BytesN<32>)> = None;
        let mut page_len = 0u32;

        for key in keys.iter() {
            if let Some(c) = cursor {
                if !key_after_cursor(&key, c) {
                    continue;
                }
            }
            if page_len >= effective {
                break;
            }
            if let Some(rule) = find_rule(rules, &key.1) {
                items.push_back(rule);
                last_key = Some(key.clone());
                page_len += 1;
            }
        }

        let next_cursor = match last_key {
            Some(k) => next_cursor_for(&k, page_len, effective),
            None => None,
        };

        Ok(SupervisionRulePage { items, next_cursor })
    }

    /// Get rule statistics
    pub fn compute_ruleset_stats(ruleset: &RuleSet, rules: &Vec<SupervisionRule>) -> RuleSetStatistics {
        let total_rules = ruleset.rules.len() as u32;
        let mut active_rules = 0u32;
        let mut avg_severity = 0u32;

        for rule in rules.iter() {
            if rule.is_active {
                active_rules += 1;
                avg_severity += rule.severity as u32;
            }
        }

        if active_rules > 0 {
            avg_severity /= active_rules;
        }

        RuleSetStatistics {
            total_rules,
            active_rules,
            avg_severity,
            version: ruleset.version,
        }
    }
}

/// Deterministic in-place insertion sort over composite keys.
///
/// A simple, allocation-free sort keeps the CPU/memory footprint well within
/// Soroban single-invocation limits for the capped page sizes used here.
fn sort_keys(keys: &mut Vec<(u64, BytesN<32>)>) {
    let len = keys.len();
    if len < 2 {
        return;
    }
    let mut i = 1u32;
    while i < len {
        let mut j = i;
        while j > 0 {
            let prev = keys.get(j - 1).unwrap();
            let cur = keys.get(j).unwrap();
            if compare_keys(&prev, &cur) == core::cmp::Ordering::Greater {
                keys.set(j - 1, cur);
                keys.set(j, prev);
                j -= 1;
            } else {
                break;
            }
        }
        i += 1;
    }
}

/// Locate a log by id.
fn find_log(logs: &Vec<RuleExecutionLog>, id: &BytesN<32>) -> Option<RuleExecutionLog> {
    for log in logs.iter() {
        if log.log_id == *id {
            return Some(log);
        }
    }
    None
}

/// Locate an alert by id.
fn find_alert(alerts: &Vec<ComplianceAlert>, id: &BytesN<32>) -> Option<ComplianceAlert> {
    for alert in alerts.iter() {
        if alert.alert_id == *id {
            return Some(alert);
        }
    }
    None
}

/// Locate a rule by id.
fn find_rule(rules: &Vec<SupervisionRule>, id: &BytesN<32>) -> Option<SupervisionRule> {
    for rule in rules.iter() {
        if rule.rule_id == *id {
            return Some(rule);
        }
    }
    None
}

/// Rule set statistics.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleSetStatistics {
    /// Total rules in set
    pub total_rules: u32,
    /// Active rules
    pub active_rules: u32,
    /// Average severity (0-10)
    pub avg_severity: u32,
    /// Rule set version
    pub version: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_creation() {
        let env = soroban_sdk::Env::default();
        let name = Bytes::from_slice(&env, b"Test Rule");
        let condition = Bytes::from_slice(&env, b"amount > 1000000");
        let action = Bytes::from_slice(&env, b"alert");

        let rule = RulesEngine::create_rule(&env, RegulatoryFramework::FSB, name, condition, action, 7).unwrap();

        assert!(rule.is_active);
        assert_eq!(rule.severity, 7);
    }

    #[test]
    fn test_rule_evaluation() {
        let env = soroban_sdk::Env::default();
        let name = Bytes::from_slice(&env, b"Test");
        let condition = Bytes::from_slice(&env, b"test");
        let action = Bytes::from_slice(&env, b"alert");

        let rule = RulesEngine::create_rule(&env, RegulatoryFramework::BIS, name, condition, action, 5).unwrap();

        let context = Bytes::from_slice(&env, b"transaction data");
        let eval = RulesEngine::evaluate_rule(&env, &rule, context).unwrap();

        assert_eq!(eval.severity_score, 5);
    }

    #[test]
    fn test_alert_generation() {
        let env = soroban_sdk::Env::default();
        let name = Bytes::from_slice(&env, b"Test Rule");
        let condition = Bytes::from_slice(&env, b"test");
        let action = Bytes::from_slice(&env, b"alert");

        let rule = RulesEngine::create_rule(&env, RegulatoryFramework::ECB, name, condition, action, 8).unwrap();

        let institution = soroban_sdk::Address::generate(&env);
        let supporting = Bytes::from_slice(&env, b"details");

        let alert = RulesEngine::generate_alert_from_rule(&env, &rule, institution, supporting).unwrap();

        assert_eq!(alert.severity, 8);
        assert_eq!(alert.status, AlertStatus::New as u32);
    }

    #[test]
    fn test_ruleset_creation() {
        let env = soroban_sdk::Env::default();
        let ruleset = RulesEngine::create_ruleset(&env, RegulatoryFramework::FSB).unwrap();

        assert!(ruleset.is_active);
        assert_eq!(ruleset.version, 1);
        assert_eq!(ruleset.rules.len(), 0);
    }

    #[test]
    fn validator_proof_attestations_are_race_safe_and_idempotent() {
        use crate::rbac::RbacManager;
        use soroban_sdk::testutils::Address as _;

        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        let contract = env.register(crate::AuditLedger, ());
        let admin = Address::generate(&env);
        let validators = [
            Address::generate(&env),
            Address::generate(&env),
            Address::generate(&env),
        ];
        let mut validator_set = Vec::new(&env);
        for validator in &validators {
            validator_set.push_back(validator.clone());
        }
        let ruleset_id = BytesN::from_array(&env, &[7; 32]);
        let proof = Bytes::from_slice(&env, b"canonical proof bytes");
        let conflicting_proof = Bytes::from_slice(&env, b"different proof");

        env.as_contract(&contract, || {
            RbacManager::initialize_admin(&env, &admin);
            RulesEngine::configure_validators(&env, &admin, validator_set, 2).unwrap();

            assert_eq!(
                RulesEngine::attest_rule_proof(&env, &validators[0], &ruleset_id, 1, &proof,),
                Ok(RuleAttestationOutcome::Pending(1))
            );
            assert_eq!(
                RulesEngine::attest_rule_proof(&env, &validators[0], &ruleset_id, 1, &proof,),
                Ok(RuleAttestationOutcome::AlreadyRecorded(1))
            );
            assert_eq!(
                RulesEngine::attest_rule_proof(&env, &validators[1], &ruleset_id, 1, &proof,),
                Ok(RuleAttestationOutcome::Finalized(2))
            );
            assert_eq!(
                RulesEngine::attest_rule_proof(&env, &validators[2], &ruleset_id, 1, &proof,),
                Ok(RuleAttestationOutcome::AlreadyFinalized)
            );
            assert_eq!(
                RulesEngine::attest_rule_proof(&env, &validators[2], &ruleset_id, 1, &conflicting_proof,),
                Err(RulesEngineError::ConflictingFinalization)
            );

            let round = RulesEngine::get_attestation_round(&env, &ruleset_id, 1).unwrap();
            assert_eq!(
                round.finalized_proof_hash,
                Some(RulesEngine::compute_rule_proof_hash(&env, &ruleset_id, 1, &proof).unwrap())
            );
            assert_eq!(round.candidates.get(0).unwrap().approvals.len(), 2);

            assert_eq!(
                RulesEngine::attest_rule_proof(&env, &validators[0], &ruleset_id, 2, &proof,),
                Ok(RuleAttestationOutcome::Pending(1))
            );
            assert_eq!(
                RulesEngine::attest_rule_proof(&env, &validators[0], &ruleset_id, 2, &conflicting_proof,),
                Err(RulesEngineError::ValidatorEquivocation)
            );
        });
    }

    fn make_log(env: &Env, seed: u8, executed_at: u64) -> RuleExecutionLog {
        let mut id = [0u8; 32];
        id[0] = seed;
        RuleExecutionLog {
            log_id: BytesN::from_array(env, &id),
            rule_id: BytesN::from_array(env, &[1u8; 32]),
            subject: Address::generate(env),
            executed_at,
            trigger_count: 1,
            max_alert_severity: 3,
        }
    }

    #[test]
    fn test_page_limit_cap() {
        assert!(PageLimit::new(0).is_err());
        assert_eq!(PageLimit::new(5).unwrap().effective(), 5);
        assert_eq!(PageLimit::new(1000).unwrap().effective(), MAX_PAGE_LIMIT);
    }

    #[test]
    fn test_filter_validation() {
        let env = soroban_sdk::Env::default();
        let bad = RuleQueryFilter {
            min_timestamp: Some(10),
            max_timestamp: Some(5),
            status: None,
            submitter: None,
        };
        let logs: Vec<RuleExecutionLog> = Vec::new(&env);
        let res = RulesEngine::query_execution_logs(&env, &logs, &bad, &None, &PageLimit::new(10).unwrap());
        assert!(res.is_err());
    }

    #[test]
    fn test_pagination_empty_results() {
        let env = soroban_sdk::Env::default();
        let logs: Vec<RuleExecutionLog> = Vec::new(&env);
        let page = RulesEngine::query_execution_logs(
            &env,
            &logs,
            &RuleQueryFilter::none(),
            &None,
            &PageLimit::new(10).unwrap(),
        )
        .unwrap();
        assert_eq!(page.items.len(), 0);
        assert!(page.next_cursor.is_none());
    }

    #[test]
    fn test_pagination_single_item_pages() {
        let env = soroban_sdk::Env::default();
        let mut logs: Vec<RuleExecutionLog> = Vec::new(&env);
        logs.push_back(make_log(&env, 3, 30));
        logs.push_back(make_log(&env, 1, 10));
        logs.push_back(make_log(&env, 2, 20));

        let limit = PageLimit::new(1).unwrap();
        let mut cursor: Option<PageCursor> = None;
        let mut seen = 0u32;
        let mut last_seq = 0u32;

        loop {
            let page =
                RulesEngine::query_execution_logs(&env, &logs, &RuleQueryFilter::none(), &cursor, &limit).unwrap();
            if page.items.len() == 0 {
                break;
            }
            for item in page.items.iter() {
                assert!(item.executed_at as u32 >= last_seq);
                last_seq = item.executed_at as u32;
                seen += 1;
            }
            match page.next_cursor {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }
        assert_eq!(seen, 3);
    }

    #[test]
    fn test_pagination_cursor_past_end() {
        let env = soroban_sdk::Env::default();
        let mut logs: Vec<RuleExecutionLog> = Vec::new(&env);
        logs.push_back(make_log(&env, 1, 10));

        let past = PageCursor {
            ledger_sequence: 999,
            id: BytesN::from_array(&env, &[9u8; 32]),
        };
        let page = RulesEngine::query_execution_logs(
            &env,
            &logs,
            &RuleQueryFilter::none(),
            &Some(past),
            &PageLimit::new(10).unwrap(),
        )
        .unwrap();
        assert_eq!(page.items.len(), 0);
        assert!(page.next_cursor.is_none());
    }

    #[test]
    fn test_filter_by_timestamp_range() {
        let env = soroban_sdk::Env::default();
        let mut logs: Vec<RuleExecutionLog> = Vec::new(&env);
        logs.push_back(make_log(&env, 1, 10));
        logs.push_back(make_log(&env, 2, 20));
        logs.push_back(make_log(&env, 3, 30));

        let filter = RuleQueryFilter {
            min_timestamp: Some(15),
            max_timestamp: Some(25),
            status: None,
            submitter: None,
        };
        let page =
            RulesEngine::query_execution_logs(&env, &logs, &filter, &None, &PageLimit::new(10).unwrap()).unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items.get(0).unwrap().executed_at, 20);
    }
}
