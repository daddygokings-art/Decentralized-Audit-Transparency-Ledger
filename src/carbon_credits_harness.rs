//! Test-only contract harness for the `carbon_credits` module.
//!
//! The carbon-credit functions are plain module functions that rely on
//! `Address::require_auth` and persistent storage, both of which the Soroban
//! host only permits inside a real contract invocation frame. This harness
//! registers a thin contract that forwards every entry point to the module and
//! exposes free-function shims with the module's own signatures, so tests can
//! call `issue_carbon_credit(&env, ...)` exactly as production callers do while
//! still exercising genuine auth, storage, and budget metering.

use crate::carbon_credits as cc;
use crate::rbac::{RbacManager, Role};
use soroban_sdk::testutils::Ledger as _;
use soroban_sdk::{contract, contractimpl, Address, Bytes, BytesN, Env, String, Vec};

pub use crate::carbon_credits::{
    CarbonCredit, CarbonReductionReport,
    ComplianceStandard, CreditStatus, Offset, PortfolioStatus, RegistryEntry, RenewableEnergySource,
    RenewableEnergyType, SustainabilityClaim, VerificationRecord,
};

/// Fixed harness address so shims can reach the contract from any `&Env`.
const HARNESS_ID: &str = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM";

/// Ledger time used by every harness environment (non-zero so that period
/// arithmetic such as `now - 86_400` never underflows).
pub const GENESIS_TIMESTAMP: u64 = 1_700_000_000;

#[contract]
pub struct CarbonHarness;

#[contractimpl]
impl CarbonHarness {
    pub fn rbac_init(env: Env, admin: Address) {
        RbacManager::initialize_admin(&env, &admin);
    }

    pub fn rbac_set_role(env: Env, admin: Address, target: Address, role: Role) {
        RbacManager::set_role(&env, &admin, &target, role).unwrap();
    }

    pub fn issue(
        env: Env,
        issuer: Address,
        carbon_tonnes: u32,
        source: RenewableEnergySource,
        offset: Offset,
        registry: RegistryEntry,
        standard: ComplianceStandard,
    ) -> BytesN<32> {
        cc::issue_carbon_credit(&env, issuer, carbon_tonnes, source, offset, registry, standard)
    }

    pub fn verify_energy(env: Env, credit_id: BytesN<32>, verifier: Address, mwh: u32) -> bool {
        cc::verify_renewable_energy(&env, credit_id, verifier, mwh)
    }

    pub fn calc_offset(env: Env, mwh: u32) -> u32 {
        cc::calculate_offset(&env, mwh)
    }

    pub fn tokenize(
        env: Env,
        credit_id: BytesN<32>,
        owner: Address,
        tokens: u128,
        market_value: u32,
    ) -> BytesN<32> {
        cc::tokenize_credit(&env, credit_id, owner, tokens, market_value)
    }

    pub fn retire(env: Env, caller: Address, credit_id: BytesN<32>, reason: Bytes) -> bool {
        cc::retire_credit(&env, caller, credit_id, reason)
    }

    pub fn is_retired(env: Env, credit_id: BytesN<32>) -> bool {
        cc::check_retirement_status(&env, credit_id)
    }

    pub fn transfer(env: Env, credit_id: BytesN<32>, from: Address, to: Address) -> bool {
        cc::transfer_credit(&env, credit_id, from, to)
    }

    pub fn verify_claim(env: Env, claim: SustainabilityClaim, verifier: Address) -> bool {
        cc::verify_sustainability_claim(&env, claim, verifier)
    }

    pub fn audit(
        env: Env,
        credit_id: BytesN<32>,
        auditor: Address,
        measured: u32,
    ) -> VerificationRecord {
        cc::audit_renewable_usage(&env, credit_id, auditor, measured)
    }

    pub fn verify_offset(env: Env, credit_id: BytesN<32>, verifier: Address) -> bool {
        cc::verify_offset_authenticity(&env, credit_id, verifier)
    }

    pub fn register(env: Env, caller: Address, credit_id: BytesN<32>, registry_id: Bytes) -> bool {
        cc::register_credit(&env, caller, credit_id, registry_id)
    }

    pub fn update_registry(env: Env, registry_id: Bytes, verified_by: Address) -> bool {
        cc::update_registry(&env, registry_id, verified_by)
    }

    pub fn link_standard(
        env: Env,
        caller: Address,
        credit_id: BytesN<32>,
        standard: ComplianceStandard,
    ) -> bool {
        cc::link_to_standard(&env, caller, credit_id, standard)
    }

    pub fn registry_ok(env: Env, credit_id: BytesN<32>) -> bool {
        cc::verify_registry_compliance(&env, credit_id)
    }

    pub fn reduction(env: Env, credit_id: BytesN<32>) -> u32 {
        cc::calculate_carbon_reduction(&env, credit_id)
    }

    pub fn offset_report(env: Env, start: u64, end: u64) -> CarbonReductionReport {
        cc::generate_offset_report(&env, start, end)
    }

    pub fn portfolio(env: Env, holder: Address) -> PortfolioStatus {
        cc::get_portfolio_status(&env, holder)
    }

    pub fn validate_claim(env: Env, claim: SustainabilityClaim) -> bool {
        cc::validate_claim(&env, claim)
    }

    pub fn standard_ok(env: Env, credit_id: BytesN<32>, standard: ComplianceStandard) -> bool {
        cc::verify_standard_compliance(&env, credit_id, standard)
    }

    pub fn integrity_ok(env: Env, credit_id: BytesN<32>) -> bool {
        cc::check_data_integrity(&env, credit_id)
    }

    pub fn total_retired(env: Env) -> u32 {
        cc::get_total_retired_co2e(&env)
    }

    pub fn status(env: Env, credit_id: BytesN<32>) -> CreditStatus {
        cc::get_credit_status(&env, credit_id)
    }

    pub fn details(env: Env, credit_id: BytesN<32>) -> CarbonCredit {
        cc::get_credit_details(&env, credit_id)
    }

    pub fn issuer_credits(env: Env, issuer: Address) -> Vec<BytesN<32>> {
        cc::get_issuer_credits(&env, issuer)
    }

    pub fn holder_credits(env: Env, holder: Address) -> Vec<BytesN<32>> {
        cc::get_holder_credits(&env, holder)
    }

    pub fn total_issued(env: Env) -> u32 {
        cc::get_total_credits_issued(&env)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Environment helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Create an environment with the harness registered, auth mocked, and a
/// non-zero ledger timestamp.
pub fn setup_env() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(GENESIS_TIMESTAMP);
    env.register_at(&harness_address(&env), CarbonHarness, ());
    env
}

pub fn harness_address(env: &Env) -> Address {
    Address::from_string(&String::from_str(env, HARNESS_ID))
}

pub fn client(env: &Env) -> CarbonHarnessClient<'_> {
    CarbonHarnessClient::new(env, &harness_address(env))
}

/// Bootstrap `admin` as RBAC admin and grant `target` the given `role`.
pub fn setup_rbac(env: &Env, admin: &Address, target: &Address, role: Role) {
    let c = client(env);
    c.rbac_init(admin);
    c.rbac_set_role(admin, target, &role);
}

// ─────────────────────────────────────────────────────────────────────────────
// Module-signature shims (route through the harness contract)
// ─────────────────────────────────────────────────────────────────────────────

pub fn issue_carbon_credit(
    env: &Env,
    issuer: Address,
    carbon_tonnes: u32,
    renewable_source: RenewableEnergySource,
    offset: Offset,
    registry: RegistryEntry,
    standard: ComplianceStandard,
) -> BytesN<32> {
    client(env).issue(&issuer, &carbon_tonnes, &renewable_source, &offset, &registry, &standard)
}

pub fn verify_renewable_energy(env: &Env, credit_id: BytesN<32>, verifier: Address, mwh: u32) -> bool {
    client(env).verify_energy(&credit_id, &verifier, &mwh)
}

pub fn calculate_offset(env: &Env, energy_mwh: u32) -> u32 {
    client(env).calc_offset(&energy_mwh)
}

pub fn tokenize_credit(
    env: &Env,
    credit_id: BytesN<32>,
    token_owner: Address,
    tokens_to_issue: u128,
    market_value: u32,
) -> BytesN<32> {
    client(env).tokenize(&credit_id, &token_owner, &tokens_to_issue, &market_value)
}

pub fn retire_credit(env: &Env, caller: Address, credit_id: BytesN<32>, reason: Bytes) -> bool {
    client(env).retire(&caller, &credit_id, &reason)
}

pub fn check_retirement_status(env: &Env, credit_id: BytesN<32>) -> bool {
    client(env).is_retired(&credit_id)
}

pub fn transfer_credit(env: &Env, credit_id: BytesN<32>, from: Address, to: Address) -> bool {
    client(env).transfer(&credit_id, &from, &to)
}

pub fn verify_sustainability_claim(env: &Env, claim: SustainabilityClaim, verifier: Address) -> bool {
    client(env).verify_claim(&claim, &verifier)
}

pub fn audit_renewable_usage(
    env: &Env,
    credit_id: BytesN<32>,
    auditor: Address,
    measured_energy: u32,
) -> VerificationRecord {
    client(env).audit(&credit_id, &auditor, &measured_energy)
}

pub fn verify_offset_authenticity(env: &Env, credit_id: BytesN<32>, verifier: Address) -> bool {
    client(env).verify_offset(&credit_id, &verifier)
}

pub fn register_credit(env: &Env, caller: Address, credit_id: BytesN<32>, registry_id: Bytes) -> bool {
    client(env).register(&caller, &credit_id, &registry_id)
}

pub fn update_registry(env: &Env, registry_id: Bytes, verified_by: Address) -> bool {
    client(env).update_registry(&registry_id, &verified_by)
}

pub fn link_to_standard(
    env: &Env,
    caller: Address,
    credit_id: BytesN<32>,
    standard: ComplianceStandard,
) -> bool {
    client(env).link_standard(&caller, &credit_id, &standard)
}

pub fn verify_registry_compliance(env: &Env, credit_id: BytesN<32>) -> bool {
    client(env).registry_ok(&credit_id)
}

pub fn calculate_carbon_reduction(env: &Env, credit_id: BytesN<32>) -> u32 {
    client(env).reduction(&credit_id)
}

pub fn generate_offset_report(env: &Env, start_date: u64, end_date: u64) -> CarbonReductionReport {
    client(env).offset_report(&start_date, &end_date)
}

pub fn get_portfolio_status(env: &Env, holder: Address) -> PortfolioStatus {
    client(env).portfolio(&holder)
}

pub fn validate_claim(env: &Env, claim: SustainabilityClaim) -> bool {
    client(env).validate_claim(&claim)
}

pub fn verify_standard_compliance(env: &Env, credit_id: BytesN<32>, standard: ComplianceStandard) -> bool {
    client(env).standard_ok(&credit_id, &standard)
}

pub fn check_data_integrity(env: &Env, credit_id: BytesN<32>) -> bool {
    client(env).integrity_ok(&credit_id)
}

pub fn get_total_retired_co2e(env: &Env) -> u32 {
    client(env).total_retired()
}

pub fn get_credit_status(env: &Env, credit_id: BytesN<32>) -> CreditStatus {
    client(env).status(&credit_id)
}

pub fn get_credit_details(env: &Env, credit_id: BytesN<32>) -> CarbonCredit {
    client(env).details(&credit_id)
}

pub fn get_issuer_credits(env: &Env, issuer: Address) -> Vec<BytesN<32>> {
    client(env).issuer_credits(&issuer)
}

pub fn get_holder_credits(env: &Env, holder: Address) -> Vec<BytesN<32>> {
    client(env).holder_credits(&holder)
}

pub fn get_total_credits_issued(env: &Env) -> u32 {
    client(env).total_issued()
}
