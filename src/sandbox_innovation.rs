#![no_std]

use soroban_sdk::{contracttype, Address, Bytes, BytesN, Env, Vec};

/// Innovation metric tracking.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InnovationMetrics {
    /// Participant ID
    pub participant_id: BytesN<32>,
    /// Innovation impact score (0-100)
    pub impact_score: u32,
    /// Market readiness score (0-100)
    pub market_readiness: u32,
    /// Technology maturity score (0-100)
    pub tech_maturity: u32,
    /// User adoption (0-100)
    pub user_adoption: u32,
    /// Learning outcomes (serialized)
    pub learning_outcomes: Bytes,
    /// Lessons learned
    pub lessons_learned: Bytes,
    /// Deployment readiness (true/false)
    pub deployment_ready: bool,
}

impl InnovationMetrics {
    pub fn overall_innovation_score(&self) -> u32 {
        (self.impact_score as u64
            + self.market_readiness as u64
            + self.tech_maturity as u64
            + self.user_adoption as u64) / 4 as u64 as u32
    }

    pub fn is_ready_for_mainnet(&self) -> bool {
        self.overall_innovation_score() >= 75 && self.deployment_ready
    }
}

/// Innovation tracking manager.
pub struct InnovationTracker;

impl InnovationTracker {
    /// Create innovation metrics
    pub fn create_innovation_metrics(
        participant_id: BytesN<32>,
    ) -> InnovationMetrics {
        InnovationMetrics {
            participant_id,
            impact_score: 0,
            market_readiness: 0,
            tech_maturity: 0,
            user_adoption: 0,
            learning_outcomes: Bytes::new(&soroban_sdk::Env::default()),
            lessons_learned: Bytes::new(&soroban_sdk::Env::default()),
            deployment_ready: false,
        }
    }

    /// Update innovation score
    pub fn update_scores(
        metrics: &mut InnovationMetrics,
        impact: u32,
        readiness: u32,
        maturity: u32,
        adoption: u32,
    ) -> Result<(), &'static str> {
        if impact > 100 || readiness > 100 || maturity > 100 || adoption > 100 {
            return Err("All scores must be 0-100");
        }

        metrics.impact_score = impact;
        metrics.market_readiness = readiness;
        metrics.tech_maturity = maturity;
        metrics.user_adoption = adoption;

        Ok(())
    }

    /// Mark deployment ready
    pub fn set_deployment_ready(metrics: &mut InnovationMetrics, ready: bool) {
        metrics.deployment_ready = ready;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_innovation_metrics_creation() {
        let metrics = InnovationTracker::create_innovation_metrics(BytesN::zero());
        assert_eq!(metrics.impact_score, 0);
        assert!(!metrics.deployment_ready);
    }

    #[test]
    fn test_overall_score_calculation() {
        let mut metrics = InnovationTracker::create_innovation_metrics(BytesN::zero());
        InnovationTracker::update_scores(&mut metrics, 80, 75, 85, 70).unwrap();

        let overall = metrics.overall_innovation_score();
        assert_eq!(overall, 77); // Average
    }

    #[test]
    fn test_mainnet_readiness() {
        let mut metrics = InnovationTracker::create_innovation_metrics(BytesN::zero());
        InnovationTracker::update_scores(&mut metrics, 85, 80, 90, 75).unwrap();
        InnovationTracker::set_deployment_ready(&mut metrics, true);

        assert!(metrics.is_ready_for_mainnet());
    }
}
