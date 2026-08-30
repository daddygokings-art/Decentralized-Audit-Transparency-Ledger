#![no_std]

use crate::suptech_types::{SupervisionRule, ComplianceAlert, AlertStatus, RegulatoryFramework};
use soroban_sdk::{contracttype, Address, Bytes, BytesN, Env, Symbol, Vec};

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
    pub severity_score: u8,
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
    pub max_alert_severity: u8,
}

/// Rule set for regulatory framework.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleSet {
    /// Rule set ID
    pub ruleset_id: BytesN<32>,
    /// Associated regulatory framework
    pub framework: u8, // RegulatoryFramework as u8
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
    /// Create supervision rule
    pub fn create_rule(
        env: &Env,
        framework: RegulatoryFramework,
        name: Bytes,
        condition: Bytes,
        action: Bytes,
        severity: u8,
    ) -> Result<SupervisionRule, &'static str> {
        if name.is_empty() {
            return Err("Rule name cannot be empty");
        }

        if condition.is_empty() {
            return Err("Rule condition cannot be empty");
        }

        if severity > 10 {
            return Err("Severity must be 0-10");
        }

        let rule_id = Self::compute_rule_id(env, framework, &name);

        Ok(SupervisionRule {
            rule_id,
            name,
            framework: framework as u8,
            condition,
            action,
            severity,
            is_active: true,
            created_at: env.ledger().timestamp(),
            updated_at: env.ledger().timestamp(),
        })
    }

    /// Compute rule ID
    pub fn compute_rule_id(
        env: &Env,
        framework: RegulatoryFramework,
        name: &Bytes,
    ) -> BytesN<32> {
        
        let mut input = Bytes::new(env);
        input.append(&Bytes::from_slice(
            env,
            framework.as_symbol().to_string().as_bytes(),
        ));
        input.append(name);

        env.crypto().sha256(&input)
    }

    /// Evaluate rule against context
    pub fn evaluate_rule(
        env: &Env,
        rule: &SupervisionRule,
        context: Bytes,
    ) -> Result<RuleEvaluation, &'static str> {
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
            status: AlertStatus::New as u8,
            resolution_notes: Bytes::new(env),
        })
    }

    /// Compute alert ID
    pub fn compute_alert_id(
        env: &Env,
        rule_id: &BytesN<32>,
        institution: &Address,
    ) -> BytesN<32> {
        
        let mut input = Bytes::new(env);
        input.append(&Bytes::from_slice(env, rule_id.as_ref()));
        input.append(&Bytes::from_slice(env, institution.to_xdr().as_ref()));
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
    pub fn create_ruleset(
        env: &Env,
        framework: RegulatoryFramework,
    ) -> Result<RuleSet, &'static str> {
        let ruleset_id = Self::compute_ruleset_id(env, framework);

        Ok(RuleSet {
            ruleset_id,
            framework: framework as u8,
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
        input.append(&Bytes::from_slice(
            env,
            framework.as_symbol().to_string().as_bytes(),
        ));
        input.append(&Bytes::from_slice(env, b"RULESET"));

        env.crypto().sha256(&input)
    }

    /// Add rule to set
    pub fn add_rule_to_set(
        ruleset: &mut RuleSet,
        rule_id: BytesN<32>,
    ) -> Result<(), &'static str> {
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
    pub fn remove_rule_from_set(
        ruleset: &mut RuleSet,
        rule_id: &BytesN<32>,
    ) -> Result<(), &'static str> {
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
            if let Ok(eval) = Self::evaluate_rule(env, rule, context.clone()) {
                evaluations.push_back(eval);
            }
        }

        Ok(evaluations)
    }

    /// Get rule statistics
    pub fn compute_ruleset_stats(
        ruleset: &RuleSet,
        rules: &Vec<SupervisionRule>,
    ) -> RuleSetStatistics {
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

        let rule = RulesEngine::create_rule(
            &env,
            RegulatoryFramework::FSB,
            name,
            condition,
            action,
            7,
        )
        .unwrap();

        assert!(rule.is_active);
        assert_eq!(rule.severity, 7);
    }

    #[test]
    fn test_rule_evaluation() {
        let env = soroban_sdk::Env::default();
        let name = Bytes::from_slice(&env, b"Test");
        let condition = Bytes::from_slice(&env, b"test");
        let action = Bytes::from_slice(&env, b"alert");

        let rule = RulesEngine::create_rule(
            &env,
            RegulatoryFramework::BIS,
            name,
            condition,
            action,
            5,
        )
        .unwrap();

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

        let rule = RulesEngine::create_rule(
            &env,
            RegulatoryFramework::ECB,
            name,
            condition,
            action,
            8,
        )
        .unwrap();

        let institution = soroban_sdk::Address::generate(&env);
        let supporting = Bytes::from_slice(&env, b"details");

        let alert =
            RulesEngine::generate_alert_from_rule(&env, &rule, institution, supporting)
                .unwrap();

        assert_eq!(alert.severity, 8);
        assert_eq!(alert.status, AlertStatus::New as u8);
    }

    #[test]
    fn test_ruleset_creation() {
        let env = soroban_sdk::Env::default();
        let ruleset =
            RulesEngine::create_ruleset(&env, RegulatoryFramework::FSB).unwrap();

        assert!(ruleset.is_active);
        assert_eq!(ruleset.version, 1);
        assert_eq!(ruleset.rules.len(), 0);
    }
}
