//! Contract Event Zero-Trust Architecture Module
//!
//! Enforces Zero-Trust security principles for contract events and off-chain services:
//! - Identity-based access control (SPIFFE/SPIRE identities & cryptographic proofs)
//! - Device trust scoring and hardware posture attestation
//! - Network segmentation & microsegmentation boundaries
//! - Continuous verification & dynamic session risk evaluation
//! - Fine-grained least-privilege capability grants

use soroban_sdk::{
    contracttype, Address, Bytes, BytesN, Env, Symbol, Vec,
};

/// Trust tier assigned to a device or workload
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TrustTier {
    /// Untrusted or unauthenticated entity
    Untrusted = 0,
    /// Low trust (basic authentication without verified device posture)
    Low = 1,
    /// Medium trust (valid identity and standard compliant device)
    Medium = 2,
    /// High trust (hardware TPM/Secure Enclave attestation & managed device)
    High = 3,
    /// Verified Zero-Trust (continuous verification passed with zero anomalies)
    VerifiedZeroTrust = 4,
}

/// Network segment classifications for microsegmentation
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum NetworkSegment {
    /// Public internet edge / Ingress
    PublicEdge = 0,
    /// Demilitarized zone / API Gateway
    DMZ = 1,
    /// Internal application and indexing services
    ApplicationCore = 2,
    /// Secret storage, Vault, and HSM key managers
    SecureVault = 3,
    /// Validator and consensus engine
    ConsensusEngine = 4,
}

/// Workload identity credential based on SPIFFE standard
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadIdentity {
    /// SPIFFE ID URI (e.g. "spiffe://auditledger.org/ns/prod/sa/relayer")
    pub spiffe_id: Bytes,
    /// Associated public key / address
    pub principal_address: Address,
    /// Trust domain identifier
    pub trust_domain: Symbol,
    /// Issuance timestamp
    pub issued_at: u64,
    /// Expiration timestamp
    pub expires_at: u64,
    /// Cryptographic signature / token hash
    pub token_digest: BytesN<32>,
}

/// Device security posture record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevicePosture {
    /// Unique hardware/device fingerprint hash
    pub device_id: BytesN<32>,
    /// OS platform string
    pub platform: Symbol,
    /// Hardware TPM / Secure Enclave attestation verified
    pub has_hardware_tpm: bool,
    /// Full disk encryption enabled
    pub is_disk_encrypted: bool,
    /// Endpoint Detection and Response (EDR) agent active
    pub is_edr_active: bool,
    /// Jailbreak / root detection status (true = clean/unrooted)
    pub is_uncompromised: bool,
    /// Calculated posture trust score (0 - 100)
    pub posture_score: u32,
    /// Timestamp of last posture verification
    pub verified_at: u64,
}

/// Continuous verification session state
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContinuousSession {
    /// Unique session identifier
    pub session_id: BytesN<32>,
    /// Authenticated principal address
    pub principal: Address,
    /// Associated device ID
    pub device_id: BytesN<32>,
    /// Current risk score (0 = no risk, 100 = critical threat)
    pub dynamic_risk_score: u32,
    /// Assigned trust tier
    pub trust_tier: TrustTier,
    /// Session initiation timestamp
    pub started_at: u64,
    /// Last verified heartbeat timestamp
    pub last_heartbeat_at: u64,
    /// Maximum allowed session duration in seconds
    pub max_lifetime_seconds: u64,
    /// Explicit revocation flag
    pub is_revoked: bool,
}

/// Least-privilege capability grant
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityGrant {
    /// Unique grant ID
    pub grant_id: Symbol,
    /// Principal receiving the grant
    pub grantee: Address,
    /// Scoped capability names (e.g. "event:log", "governance:vote", "compliance:sweep")
    pub allowed_capabilities: Vec<Symbol>,
    /// Target network segment allowed
    pub target_segment: NetworkSegment,
    /// Required minimum trust tier
    pub required_trust_tier: TrustTier,
    /// Grant expiration timestamp
    pub expires_at: u64,
    /// Granting authority
    pub granted_by: Address,
}

/// Zero-Trust policy evaluation engine
pub struct ZeroTrustEngine;

impl ZeroTrustEngine {
    /// Calculate device trust posture score (0 - 100)
    pub fn calculate_posture_score(
        has_tpm: bool,
        is_encrypted: bool,
        is_edr_active: bool,
        is_uncompromised: bool,
    ) -> u32 {
        if !is_uncompromised {
            return 0; // Device compromised / rooted
        }

        let mut score = 20u32; // Base score for non-compromised device
        if has_tpm {
            score += 30;
        }
        if is_encrypted {
            score += 25;
        }
        if is_edr_active {
            score += 25;
        }
        score
    }

    /// Derive TrustTier from posture score and dynamic risk
    pub fn derive_trust_tier(posture_score: u32, risk_score: u32) -> TrustTier {
        if posture_score < 40 || risk_score >= 70 {
            TrustTier::Untrusted
        } else if posture_score < 60 || risk_score >= 50 {
            TrustTier::Low
        } else if posture_score < 80 || risk_score >= 30 {
            TrustTier::Medium
        } else if posture_score < 95 || risk_score >= 10 {
            TrustTier::High
        } else {
            TrustTier::VerifiedZeroTrust
        }
    }

    /// Validate network microsegmentation routing policy
    pub fn validate_segment_access(
        source: NetworkSegment,
        destination: NetworkSegment,
        tier: TrustTier,
    ) -> bool {
        match destination {
            NetworkSegment::PublicEdge => true,
            NetworkSegment::DMZ => tier >= TrustTier::Low,
            NetworkSegment::ApplicationCore => {
                // Cannot access ApplicationCore directly from PublicEdge
                if source == NetworkSegment::PublicEdge {
                    false
                } else {
                    tier >= TrustTier::Medium
                }
            }
            NetworkSegment::SecureVault => {
                // Only ApplicationCore or ConsensusEngine can access Vault with High trust
                (source == NetworkSegment::ApplicationCore || source == NetworkSegment::ConsensusEngine)
                    && tier >= TrustTier::High
            }
            NetworkSegment::ConsensusEngine => {
                // Strict isolation: requires High or VerifiedZeroTrust
                source == NetworkSegment::ApplicationCore && tier >= TrustTier::High
            }
        }
    }

    /// Continuously verify session validity against drift, timeout, and risk
    pub fn verify_continuous_session(
        env: &Env,
        session: &ContinuousSession,
        max_heartbeat_idle_seconds: u64,
    ) -> bool {
        if session.is_revoked {
            return false;
        }

        let current_time = env.ledger().timestamp();

        // Check lifetime expiration
        if current_time.saturating_sub(session.started_at) > session.max_lifetime_seconds {
            return false;
        }

        // Check idle heartbeat drift
        if current_time.saturating_sub(session.last_heartbeat_at) > max_heartbeat_idle_seconds {
            return false;
        }

        // Check dynamic risk threshold
        if session.dynamic_risk_score >= 80 {
            return false;
        }

        true
    }

    /// Authorize least-privilege capability execution
    pub fn authorize_capability(
        env: &Env,
        grant: &CapabilityGrant,
        requested_capability: &Symbol,
        target_segment: NetworkSegment,
        caller_tier: TrustTier,
    ) -> bool {
        let current_time = env.ledger().timestamp();

        // Verify expiration
        if current_time >= grant.expires_at {
            return false;
        }

        // Verify trust tier requirement
        if caller_tier < grant.required_trust_tier {
            return false;
        }

        // Verify network segment allowance
        if grant.target_segment != target_segment && grant.target_segment != NetworkSegment::ApplicationCore {
            return false;
        }

        // Verify capability membership
        for cap in grant.allowed_capabilities.iter() {
            if &cap == requested_capability {
                return true;
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_posture_score_compromised() {
        let score = ZeroTrustEngine::calculate_posture_score(true, true, true, false);
        assert_eq!(score, 0);
    }

    #[test]
    fn test_posture_score_full_hardware() {
        let score = ZeroTrustEngine::calculate_posture_score(true, true, true, true);
        assert_eq!(score, 100);
    }

    #[test]
    fn test_derive_trust_tier() {
        assert_eq!(ZeroTrustEngine::derive_trust_tier(100, 5), TrustTier::VerifiedZeroTrust);
        assert_eq!(ZeroTrustEngine::derive_trust_tier(85, 20), TrustTier::High);
        assert_eq!(ZeroTrustEngine::derive_trust_tier(65, 40), TrustTier::Medium);
        assert_eq!(ZeroTrustEngine::derive_trust_tier(50, 60), TrustTier::Low);
        assert_eq!(ZeroTrustEngine::derive_trust_tier(100, 85), TrustTier::Untrusted);
    }

    #[test]
    fn test_segment_microsegmentation() {
        // PublicEdge cannot directly access SecureVault
        assert!(!ZeroTrustEngine::validate_segment_access(
            NetworkSegment::PublicEdge,
            NetworkSegment::SecureVault,
            TrustTier::VerifiedZeroTrust
        ));

        // ApplicationCore with High trust can access SecureVault
        assert!(ZeroTrustEngine::validate_segment_access(
            NetworkSegment::ApplicationCore,
            NetworkSegment::SecureVault,
            TrustTier::High
        ));

        // ApplicationCore with Low trust cannot access SecureVault
        assert!(!ZeroTrustEngine::validate_segment_access(
            NetworkSegment::ApplicationCore,
            NetworkSegment::SecureVault,
            TrustTier::Low
        ));
    }
}
