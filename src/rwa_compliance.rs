//! RWA Compliance Engine — KYC/AML management, investor profiling, and
//! compliance gating for all token operations.
//!
//! The `ComplianceEngine` validates investor eligibility before transfers,
//! manages KYC tier upgrades, runs compliance checks, and records findings
//! in an on-chain audit trail.
#![no_std]

use crate::rwa_types::{
    ComplianceCheck, ComplianceFramework, InvestorProfile, KycTier, RwaToken, TokenizationStatus,
};
use soroban_sdk::{contracttype, Address, Bytes, BytesN, Env, Symbol, Vec};

// ── Storage Keys ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum ComplianceKey {
    /// Investor profile keyed by address.
    InvestorProfile(Address),
    /// Ordered list of investor addresses.
    InvestorIndex,
    /// Compliance check record by check ID.
    CheckRecord(BytesN<32>),
    /// All check IDs for a given subject (packed).
    SubjectChecks(Bytes),
    /// Blocklist flag per address.
    Blocked(Address),
    /// Global compliance check count.
    CheckCount,
}

// ── Compliance Engine ─────────────────────────────────────────────────────────

/// Central compliance management system.
pub struct ComplianceEngine;

impl ComplianceEngine {
    // ── Investor Onboarding ───────────────────────────────────────────────────

    /// Register a new investor profile.
    pub fn register_investor(
        env: &Env,
        address: Address,
        kyc_tier: KycTier,
        aml_cleared: bool,
        is_accredited: bool,
        jurisdiction: Bytes,
        kyc_expiry: u64,
        notes: Bytes,
    ) -> Result<InvestorProfile, &'static str> {
        if jurisdiction.is_empty() {
            return Err("Jurisdiction must be provided");
        }

        // Prevent duplicate registration — update instead.
        if env
            .storage()
            .instance()
            .has(&ComplianceKey::InvestorProfile(address.clone()))
        {
            return Err("Investor already registered; use update_investor_kyc");
        }

        let now = env.ledger().timestamp();
        let profile_id = Self::compute_profile_id(env, &address);

        let profile = InvestorProfile {
            profile_id,
            address: address.clone(),
            kyc_tier: kyc_tier as u8,
            aml_cleared,
            is_accredited,
            jurisdiction,
            kyc_verified_at: now,
            kyc_expiry,
            created_at: now,
            is_restricted: false,
            notes,
        };

        env.storage()
            .instance()
            .set(&ComplianceKey::InvestorProfile(address.clone()), &profile);

        let mut index: Vec<Address> = env
            .storage()
            .instance()
            .get(&ComplianceKey::InvestorIndex)
            .unwrap_or_else(|| Vec::new(env));
        index.push_back(address.clone());
        env.storage()
            .instance()
            .set(&ComplianceKey::InvestorIndex, &index);

        env.events().publish(
            (
                Symbol::new(env, "rwa_compliance"),
                Symbol::new(env, "investor_registered"),
            ),
            (address, kyc_tier as u8),
        );

        Ok(profile)
    }

    /// Upgrade or downgrade an investor's KYC tier.
    pub fn update_investor_kyc(
        env: &Env,
        address: &Address,
        new_tier: KycTier,
        aml_cleared: bool,
        kyc_expiry: u64,
    ) -> Result<InvestorProfile, &'static str> {
        let mut profile = Self::get_investor(env, address)?;

        let old_tier = profile.kyc_tier;
        profile.kyc_tier = new_tier as u8;
        profile.aml_cleared = aml_cleared;
        profile.kyc_verified_at = env.ledger().timestamp();
        profile.kyc_expiry = kyc_expiry;

        env.storage()
            .instance()
            .set(&ComplianceKey::InvestorProfile(address.clone()), &profile);

        env.events().publish(
            (
                Symbol::new(env, "rwa_compliance"),
                Symbol::new(env, "kyc_updated"),
            ),
            (address.clone(), old_tier, new_tier as u8),
        );

        Ok(profile)
    }

    /// Restrict an investor (freeze participation).
    pub fn restrict_investor(
        env: &Env,
        address: &Address,
        reason: Bytes,
    ) -> Result<(), &'static str> {
        let mut profile = Self::get_investor(env, address)?;
        profile.is_restricted = true;
        profile.notes = reason.clone();
        env.storage()
            .instance()
            .set(&ComplianceKey::InvestorProfile(address.clone()), &profile);

        // Set explicit blocklist flag for fast transfer gating.
        env.storage()
            .instance()
            .set(&ComplianceKey::Blocked(address.clone()), &true);

        env.events().publish(
            (
                Symbol::new(env, "rwa_compliance"),
                Symbol::new(env, "investor_restricted"),
            ),
            (address.clone(), reason),
        );

        Ok(())
    }

    /// Lift the restriction on an investor.
    pub fn unrestrict_investor(env: &Env, address: &Address) -> Result<(), &'static str> {
        let mut profile = Self::get_investor(env, address)?;
        profile.is_restricted = false;
        env.storage()
            .instance()
            .set(&ComplianceKey::InvestorProfile(address.clone()), &profile);
        env.storage()
            .instance()
            .remove(&ComplianceKey::Blocked(address.clone()));
        Ok(())
    }

    // ── Compliance Checks ─────────────────────────────────────────────────────

    /// Record the result of a KYC, AML, or sanctions check.
    pub fn record_compliance_check(
        env: &Env,
        subject: Bytes,
        check_type: Symbol,
        result: Symbol,
        score: u8,
        checked_by: Address,
        finding: Bytes,
        requires_refresh: bool,
    ) -> Result<ComplianceCheck, &'static str> {
        if score > 100 {
            return Err("Compliance score must be 0-100");
        }

        let check_id = Self::compute_check_id(env, &subject, &check_type);
        let now = env.ledger().timestamp();

        let check = ComplianceCheck {
            check_id: check_id.clone(),
            subject: subject.clone(),
            check_type,
            result,
            score,
            checked_at: now,
            checked_by,
            finding,
            requires_refresh,
        };

        env.storage()
            .instance()
            .set(&ComplianceKey::CheckRecord(check_id.clone()), &check);

        // Append to per-subject index.
        let mut ids: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&ComplianceKey::SubjectChecks(subject.clone()))
            .unwrap_or_else(|| Vec::new(env));
        ids.push_back(check_id);
        env.storage()
            .instance()
            .set(&ComplianceKey::SubjectChecks(subject), &ids);

        // Increment global count.
        let count: u32 = env
            .storage()
            .instance()
            .get(&ComplianceKey::CheckCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&ComplianceKey::CheckCount, &(count + 1));

        Ok(check)
    }

    // ── Transfer Eligibility Gates ────────────────────────────────────────────

    /// Validate that a proposed transfer is compliant.
    ///
    /// Checks sender and recipient KYC, AML clearance, non-restriction,
    /// accreditation (if enforced), and token active status.
    pub fn validate_transfer(
        env: &Env,
        token: &RwaToken,
        from: &Address,
        to: &Address,
        enforce_accreditation: bool,
        enforce_aml: bool,
    ) -> Result<(), &'static str> {
        let status = TokenizationStatus::from_u8(token.status).ok_or("Invalid token status")?;
        if !status.allows_transfer() {
            return Err("Token status does not permit transfers");
        }

        let from_profile = Self::get_investor(env, from)?;
        let to_profile = Self::get_investor(env, to)?;
        let now = env.ledger().timestamp();

        if !from_profile.is_kyc_valid(now) {
            return Err("Sender KYC is invalid or expired");
        }
        if !to_profile.is_kyc_valid(now) {
            return Err("Recipient KYC is invalid or expired");
        }
        if enforce_aml && (!from_profile.aml_cleared || !to_profile.aml_cleared) {
            return Err("AML clearance required for both parties");
        }
        if enforce_accreditation && (!from_profile.is_accredited || !to_profile.is_accredited) {
            return Err("Both parties must be accredited investors");
        }

        Ok(())
    }

    // ── Queries ───────────────────────────────────────────────────────────────

    /// Retrieve an investor profile; errors if not found.
    pub fn get_investor(env: &Env, address: &Address) -> Result<InvestorProfile, &'static str> {
        env.storage()
            .instance()
            .get(&ComplianceKey::InvestorProfile(address.clone()))
            .ok_or("Investor profile not found")
    }

    /// Return the list of all registered investor addresses.
    pub fn list_investors(env: &Env) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&ComplianceKey::InvestorIndex)
            .unwrap_or_else(|| Vec::new(env))
    }

    /// `true` if the address is explicitly blocked.
    pub fn is_blocked(env: &Env, address: &Address) -> bool {
        env.storage()
            .instance()
            .get::<ComplianceKey, bool>(&ComplianceKey::Blocked(address.clone()))
            .unwrap_or(false)
    }

    /// Retrieve a specific compliance check record.
    pub fn get_check(
        env: &Env,
        check_id: &BytesN<32>,
    ) -> Result<ComplianceCheck, &'static str> {
        env.storage()
            .instance()
            .get(&ComplianceKey::CheckRecord(check_id.clone()))
            .ok_or("Compliance check not found")
    }

    /// Total compliance checks recorded.
    pub fn total_checks(env: &Env) -> u32 {
        env.storage()
            .instance()
            .get(&ComplianceKey::CheckCount)
            .unwrap_or(0)
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn compute_profile_id(env: &Env, address: &Address) -> BytesN<32> {
        use soroban_sdk::crypto::sha256;
        let mut input = Bytes::new(env);
        input.append(&Bytes::from_slice(env, address.to_xdr().as_ref()));
        input.append(&Bytes::from_slice(
            env,
            &env.ledger().timestamp().to_le_bytes(),
        ));
        sha256(&input)
    }

    fn compute_check_id(env: &Env, subject: &Bytes, check_type: &Symbol) -> BytesN<32> {
        use soroban_sdk::crypto::sha256;
        let mut input = Bytes::new(env);
        input.append(subject);
        input.append(&Bytes::from_slice(
            env,
            &env.ledger().timestamp().to_le_bytes(),
        ));
        sha256(&input)
    }
}
