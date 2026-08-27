#![no_std]

use crate::sandbox_types::*;
use soroban_sdk::{contracttype, Address, Bytes, BytesN, Env, Symbol, Vec};

/// Sandbox supervision record.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupervisionRecord {
    /// Record ID
    pub record_id: BytesN<32>,
    /// Participant being supervised
    pub participant_id: BytesN<32>,
    /// Supervisor address
    pub supervisor: Address,
    /// Inspection timestamp
    pub inspection_date: u64,
    /// Compliance findings (serialized)
    pub findings: Bytes,
    /// Overall assessment (0-100)
    pub assessment_score: u32,
    /// Risk level (low/medium/high)
    pub risk_level: u8,
    /// Corrective actions needed
    pub corrective_actions: Bytes,
}

/// Risk level enumeration.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RiskLevel {
    Low = 0,
    Medium = 1,
    High = 2,
    Critical = 3,
}

impl RiskLevel {
    pub fn requires_intervention(&self) -> bool {
        matches!(self, RiskLevel::High | RiskLevel::Critical)
    }
}

/// Sandbox supervision manager.
pub struct SupervisionManager;

impl SupervisionManager {
    /// Create supervision record
    pub fn create_supervision_record(
        env: &Env,
        participant_id: BytesN<32>,
        supervisor: Address,
        findings: Bytes,
        assessment_score: u32,
        risk_level: RiskLevel,
    ) -> Result<SupervisionRecord, &'static str> {
        if assessment_score > 100 {
            return Err("Assessment score must be 0-100");
        }

        let record_id = Self::compute_record_id(env, &participant_id);

        Ok(SupervisionRecord {
            record_id,
            participant_id,
            supervisor,
            inspection_date: env.ledger().timestamp(),
            findings,
            assessment_score,
            risk_level: risk_level as u8,
            corrective_actions: Bytes::new(env),
        })
    }

    /// Compute record ID
    pub fn compute_record_id(env: &Env, participant_id: &BytesN<32>) -> BytesN<32> {
        use soroban_sdk::crypto::sha256;

        let mut input = Bytes::new(env);
        input.append(&Bytes::from_slice(env, participant_id.as_ref()));
        input.append(&Bytes::from_slice(env, &env.ledger().timestamp().to_le_bytes()));

        sha256(&input)
    }

    /// Add corrective actions
    pub fn add_corrective_actions(
        record: &mut SupervisionRecord,
        actions: Bytes,
    ) -> Result<(), &'static str> {
        if actions.is_empty() {
            return Err("Actions cannot be empty");
        }

        record.corrective_actions = actions;
        Ok(())
    }

    /// Check if regular monitoring required
    pub fn requires_regular_monitoring(record: &SupervisionRecord) -> bool {
        let risk = match record.risk_level {
            r if r == RiskLevel::Low as u8 => RiskLevel::Low,
            r if r == RiskLevel::Medium as u8 => RiskLevel::Medium,
            r if r == RiskLevel::High as u8 => RiskLevel::High,
            r if r == RiskLevel::Critical as u8 => RiskLevel::Critical,
            _ => RiskLevel::Low,
        };

        risk.requires_intervention()
    }

    /// Compute compliance rate from multiple inspections
    pub fn compute_compliance_trend(
        records: &Vec<SupervisionRecord>,
    ) -> u32 {
        if records.len() == 0 {
            return 0;
        }

        let mut total_score = 0u64;
        for record in records.iter() {
            total_score += record.assessment_score as u64;
        }

        (total_score / records.len() as u64) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supervision_record_creation() {
        let env = soroban_sdk::Env::default();
        let participant_id = BytesN::zero();
        let supervisor = soroban_sdk::Address::generate(&env);
        let findings = Bytes::from_slice(&env, b"findings");

        let record = SupervisionManager::create_supervision_record(
            &env,
            participant_id,
            supervisor,
            findings,
            85,
            RiskLevel::Low,
        )
        .unwrap();

        assert_eq!(record.assessment_score, 85);
        assert_eq!(record.risk_level, RiskLevel::Low as u8);
    }

    #[test]
    fn test_risk_level_intervention() {
        assert!(!RiskLevel::Low.requires_intervention());
        assert!(!RiskLevel::Medium.requires_intervention());
        assert!(RiskLevel::High.requires_intervention());
        assert!(RiskLevel::Critical.requires_intervention());
    }
}
