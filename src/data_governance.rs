/// Contract Event Data Governance and Catalog Engine
///
/// Implements on-chain metadata anchoring for data governance:
/// - Searchable Data Catalog with classifications and compliance tags
/// - Cryptographic Data Lineage tracking (DAG edges and transformation hashes)
/// - Automated Data Quality Scorecards (completeness, validity, accuracy, uniqueness, timeliness)
/// - Fine-grained Access Policies (RBAC/ABAC and PII masking rules)
/// - Data Stewardship Workflows (change requests, reviews, and auditable approvals)

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, Bytes, BytesN, Env, Symbol,
    Vec, panic_with_error, log,
};

// ============================================================================
// Errors
// ============================================================================

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum GovernanceError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    AssetNotFound = 4,
    PolicyNotFound = 5,
    StewardshipRequestNotFound = 6,
    InvalidStatusTransition = 7,
    QualityThresholdFailed = 8,
    InvalidClassification = 9,
}

// ============================================================================
// Data Structures
// ============================================================================

/// Data classification levels
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum DataClassification {
    Public = 0,
    Internal = 1,
    Confidential = 2,
    Restricted = 3,
}

/// Standard regulatory compliance tags
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum ComplianceTag {
    Gdpr = 0,
    Ccpa = 1,
    Hipaa = 2,
    Esg = 3,
    Soc2 = 4,
    Pcidss = 5,
}

/// Catalog asset entity
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogAsset {
    pub asset_id: BytesN<32>,
    pub name: Symbol,
    pub description: Bytes,
    pub classification: DataClassification,
    pub owner: Address,
    pub steward: Address,
    pub tags: Vec<ComplianceTag>,
    pub version: u32,
    pub created_at: u64,
    pub updated_at: u64,
}

/// Data lineage provenance edge
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineageEdge {
    pub edge_id: BytesN<32>,
    pub source_asset_id: BytesN<32>,
    pub target_asset_id: BytesN<32>,
    pub transform_type: Symbol,
    pub transformation_hash: BytesN<32>,
    pub recorded_at: u64,
}

/// Data quality scorecard
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QualityScorecard {
    pub asset_id: BytesN<32>,
    pub completeness_bps: u32, // Basis points (10000 = 100%)
    pub validity_bps: u32,
    pub accuracy_bps: u32,
    pub uniqueness_bps: u32,
    pub timeliness_seconds: u64,
    pub total_records_evaluated: u64,
    pub passed: bool,
    pub evaluated_at: u64,
}

/// Access control and column masking policy
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessPolicy {
    pub policy_id: BytesN<32>,
    pub asset_id: BytesN<32>,
    pub grantee_role: Symbol,
    pub grantee: Option<Address>,
    pub can_read: bool,
    pub can_write: bool,
    pub can_export: bool,
    pub mask_pii: bool,
    pub expires_at: u64,
}

/// Data stewardship change / access request
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StewardshipRequest {
    pub request_id: u64,
    pub asset_id: BytesN<32>,
    pub requester: Address,
    pub action_type: Symbol,
    pub justification: Bytes,
    pub status: Symbol, // pending, approved, rejected
    pub reviewer: Option<Address>,
    pub reviewed_at: u64,
    pub created_at: u64,
}

// ============================================================================
// Storage Keys
// ============================================================================

#[contracttype]
pub enum GovernanceKey {
    Admin,
    ChiefSteward,
    AssetById(BytesN<32>),
    LineageEdge(BytesN<32>),
    QualityScorecard(BytesN<32>),
    PolicyById(BytesN<32>),
    PolicyByAssetRole(BytesN<32>, Symbol),
    StewardshipRequestById(u64),
    NextRequestId,
}

// ============================================================================
// Contract Implementation
// ============================================================================

#[contract]
pub struct DataGovernanceContract;

#[contractimpl]
impl DataGovernanceContract {
    /// Initialize the data governance contract
    pub fn initialize(env: Env, admin: Address, chief_steward: Address) -> Result<(), GovernanceError> {
        if env.storage().instance().has(&GovernanceKey::Admin) {
            return Err(GovernanceError::AlreadyInitialized);
        }

        admin.require_auth();
        env.storage().instance().set(&GovernanceKey::Admin, &admin);
        env.storage().instance().set(&GovernanceKey::ChiefSteward, &chief_steward);
        env.storage().instance().set(&GovernanceKey::NextRequestId, &1u64);

        Ok(())
    }

    /// Register a new asset in the data catalog
    pub fn register_asset(
        env: Env,
        caller: Address,
        asset: CatalogAsset,
    ) -> Result<BytesN<32>, GovernanceError> {
        caller.require_auth();

        if !env.storage().instance().has(&GovernanceKey::Admin) {
            return Err(GovernanceError::NotInitialized);
        }

        let asset_key = GovernanceKey::AssetById(asset.asset_id.clone());
        env.storage().instance().set(&asset_key, &asset);

        Ok(asset.asset_id)
    }

    /// Update catalog asset metadata
    pub fn update_asset(
        env: Env,
        caller: Address,
        asset_id: BytesN<32>,
        description: Bytes,
        classification: DataClassification,
        tags: Vec<ComplianceTag>,
    ) -> Result<(), GovernanceError> {
        caller.require_auth();

        let asset_key = GovernanceKey::AssetById(asset_id.clone());
        let mut asset: CatalogAsset = env
            .storage()
            .instance()
            .get(&asset_key)
            .ok_or(GovernanceError::AssetNotFound)?;

        if caller != asset.owner && caller != asset.steward {
            return Err(GovernanceError::Unauthorized);
        }

        let now = env.ledger().timestamp();
        asset.description = description;
        asset.classification = classification;
        asset.tags = tags;
        asset.version += 1;
        asset.updated_at = now;

        env.storage().instance().set(&asset_key, &asset);

        Ok(())
    }

    /// Record a lineage edge connecting two catalog assets
    pub fn record_lineage(
        env: Env,
        caller: Address,
        edge: LineageEdge,
    ) -> Result<BytesN<32>, GovernanceError> {
        caller.require_auth();

        let edge_key = GovernanceKey::LineageEdge(edge.edge_id.clone());
        env.storage().instance().set(&edge_key, &edge);

        Ok(edge.edge_id)
    }

    /// Record an automated data quality scorecard for an asset
    pub fn record_quality_scorecard(
        env: Env,
        caller: Address,
        scorecard: QualityScorecard,
    ) -> Result<(), GovernanceError> {
        caller.require_auth();

        let q_key = GovernanceKey::QualityScorecard(scorecard.asset_id.clone());
        env.storage().instance().set(&q_key, &scorecard);

        Ok(())
    }

    /// Set an access policy for an asset
    pub fn set_access_policy(
        env: Env,
        caller: Address,
        policy: AccessPolicy,
    ) -> Result<(), GovernanceError> {
        caller.require_auth();

        let asset: CatalogAsset = env
            .storage()
            .instance()
            .get(&GovernanceKey::AssetById(policy.asset_id.clone()))
            .ok_or(GovernanceError::AssetNotFound)?;

        if caller != asset.owner && caller != asset.steward {
            return Err(GovernanceError::Unauthorized);
        }

        let policy_key = GovernanceKey::PolicyById(policy.policy_id.clone());
        let role_key = GovernanceKey::PolicyByAssetRole(policy.asset_id.clone(), policy.grantee_role.clone());

        env.storage().instance().set(&policy_key, &policy);
        env.storage().instance().set(&role_key, &policy);

        Ok(())
    }

    /// Verify access permission for a role and action
    pub fn check_access(
        env: Env,
        asset_id: BytesN<32>,
        role: Symbol,
        action: Symbol,
    ) -> bool {
        let role_key = GovernanceKey::PolicyByAssetRole(asset_id, role);
        if let Some(policy) = env.storage().instance().get::<_, AccessPolicy>(&role_key) {
            let now = env.ledger().timestamp();
            if policy.expires_at > 0 && policy.expires_at < now {
                return false;
            }

            if action == Symbol::new(&env, "read") && policy.can_read {
                return true;
            }
            if action == Symbol::new(&env, "write") && policy.can_write {
                return true;
            }
            if action == Symbol::new(&env, "export") && policy.can_export {
                return true;
            }
        }
        false
    }

    /// Submit a data stewardship change / access request
    pub fn create_stewardship_request(
        env: Env,
        requester: Address,
        asset_id: BytesN<32>,
        action_type: Symbol,
        justification: Bytes,
    ) -> Result<u64, GovernanceError> {
        requester.require_auth();

        let next_id: u64 = env
            .storage()
            .instance()
            .get(&GovernanceKey::NextRequestId)
            .unwrap_or(1);

        let now = env.ledger().timestamp();
        let request = StewardshipRequest {
            request_id: next_id,
            asset_id,
            requester,
            action_type,
            justification,
            status: Symbol::new(&env, "pending"),
            reviewer: None,
            reviewed_at: 0,
            created_at: now,
        };

        env.storage()
            .instance()
            .set(&GovernanceKey::StewardshipRequestById(next_id), &request);
        env.storage()
            .instance()
            .set(&GovernanceKey::NextRequestId, &(next_id + 1));

        Ok(next_id)
    }

    /// Review (approve or reject) a stewardship request
    pub fn review_stewardship_request(
        env: Env,
        steward: Address,
        request_id: u64,
        approved: bool,
    ) -> Result<(), GovernanceError> {
        steward.require_auth();

        let req_key = GovernanceKey::StewardshipRequestById(request_id);
        let mut request: StewardshipRequest = env
            .storage()
            .instance()
            .get(&req_key)
            .ok_or(GovernanceError::StewardshipRequestNotFound)?;

        let now = env.ledger().timestamp();
        request.status = if approved {
            Symbol::new(&env, "approved")
        } else {
            Symbol::new(&env, "rejected")
        };
        request.reviewer = Some(steward);
        request.reviewed_at = now;

        env.storage().instance().set(&req_key, &request);

        Ok(())
    }

    /// Get catalog asset by ID
    pub fn get_asset(env: Env, asset_id: BytesN<32>) -> Option<CatalogAsset> {
        env.storage().instance().get(&GovernanceKey::AssetById(asset_id))
    }

    /// Get quality scorecard by asset ID
    pub fn get_quality_scorecard(env: Env, asset_id: BytesN<32>) -> Option<QualityScorecard> {
        env.storage().instance().get(&GovernanceKey::QualityScorecard(asset_id))
    }

    /// Get stewardship request by ID
    pub fn get_stewardship_request(env: Env, request_id: u64) -> Option<StewardshipRequest> {
        env.storage().instance().get(&GovernanceKey::StewardshipRequestById(request_id))
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn test_governance_lifecycle() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let steward = Address::generate(&env);
        let user = Address::generate(&env);

        // 1. Initialize
        assert!(DataGovernanceContract::initialize(env.clone(), admin.clone(), steward.clone()).is_ok());

        // 2. Register catalog asset
        let asset_id = BytesN::from_array(&env, &[1u8; 32]);
        let mut tags = Vec::new(&env);
        tags.push_back(ComplianceTag::Gdpr);
        tags.push_back(ComplianceTag::Soc2);

        let asset = CatalogAsset {
            asset_id: asset_id.clone(),
            name: Symbol::new(&env, "audit_events"),
            description: Bytes::new(&env),
            classification: DataClassification::Confidential,
            owner: admin.clone(),
            steward: steward.clone(),
            tags,
            version: 1,
            created_at: 100,
            updated_at: 100,
        };

        let reg_res = DataGovernanceContract::register_asset(env.clone(), admin.clone(), asset);
        assert_eq!(reg_res, Ok(asset_id.clone()));

        // 3. Quality scorecard
        let scorecard = QualityScorecard {
            asset_id: asset_id.clone(),
            completeness_bps: 9950,
            validity_bps: 10000,
            accuracy_bps: 9990,
            uniqueness_bps: 10000,
            timeliness_seconds: 2,
            total_records_evaluated: 10000,
            passed: true,
            evaluated_at: 120,
        };
        assert!(DataGovernanceContract::record_quality_scorecard(env.clone(), admin.clone(), scorecard).is_ok());
        let fetched_card = DataGovernanceContract::get_quality_scorecard(env.clone(), asset_id.clone()).unwrap();
        assert!(fetched_card.passed);

        // 4. Access policy & checking
        let policy_id = BytesN::from_array(&env, &[2u8; 32]);
        let auditor_role = Symbol::new(&env, "auditor");
        let policy = AccessPolicy {
            policy_id,
            asset_id: asset_id.clone(),
            grantee_role: auditor_role.clone(),
            grantee: None,
            can_read: true,
            can_write: false,
            can_export: true,
            mask_pii: true,
            expires_at: 0,
        };
        assert!(DataGovernanceContract::set_access_policy(env.clone(), admin.clone(), policy).is_ok());
        assert!(DataGovernanceContract::check_access(env.clone(), asset_id.clone(), auditor_role.clone(), Symbol::new(&env, "read")));
        assert!(!DataGovernanceContract::check_access(env.clone(), asset_id.clone(), auditor_role.clone(), Symbol::new(&env, "write")));

        // 5. Stewardship workflow
        let req_id = DataGovernanceContract::create_stewardship_request(
            env.clone(),
            user.clone(),
            asset_id.clone(),
            Symbol::new(&env, "access_grant"),
            Bytes::new(&env),
        ).unwrap();
        assert_eq!(req_id, 1);

        assert!(DataGovernanceContract::review_stewardship_request(env.clone(), steward.clone(), req_id, true).is_ok());
        let req = DataGovernanceContract::get_stewardship_request(env.clone(), req_id).unwrap();
        assert_eq!(req.status, Symbol::new(&env, "approved"));
    }
}
