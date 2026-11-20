//! Tests for settlement reserve monitoring and below-floor warning events.

use crate::rbac::{Rbac, Role};
use crate::settlement_reserves::*;
use crate::test_events::{count_topic, events_during};
use crate::AuditLedger;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Address, Env, Symbol};

struct Ctx {
    env: Env,
    contract: Address,
    admin: Address,
    reporter: Address,
}

fn setup() -> Ctx {
    let env = Env::default();
    // RBAC helpers call `require_auth` from inside an `as_contract` frame.
    env.mock_all_auths_allowing_non_root_auth();
    let contract = env.register(AuditLedger, ());
    let admin = Address::generate(&env);
    let reporter = Address::generate(&env);
    env.as_contract(&contract, || {
        Rbac::init_admin(&env, &admin);
        Rbac::set_role(&env, &admin, &reporter, Role::Submitter).unwrap();
    });
    Ctx {
        env,
        contract,
        admin,
        reporter,
    }
}

type Emitted = alloc::vec::Vec<soroban_sdk::xdr::ContractEvent>;

fn report_ev(
    ctx: &Ctx,
    pool: &str,
    reserves: i128,
    liabilities: i128,
) -> (Result<ReservePosition, SettlementReserveError>, Emitted) {
    events_during(&ctx.env, || {
        ctx.env.as_contract(&ctx.contract, || {
            SettlementReserveMonitor::report_reserves(
                &ctx.env,
                &ctx.reporter,
                Symbol::new(&ctx.env, pool),
                reserves,
                liabilities,
            )
        })
    })
}

fn report(ctx: &Ctx, pool: &str, reserves: i128, liabilities: i128) -> Result<ReservePosition, SettlementReserveError> {
    report_ev(ctx, pool, reserves, liabilities).0
}

#[test]
fn ratio_math() {
    assert_eq!(SettlementReserveMonitor::reserve_ratio_bps(100, 100), Ok(10_000));
    assert_eq!(SettlementReserveMonitor::reserve_ratio_bps(95, 100), Ok(9_500));
    assert_eq!(SettlementReserveMonitor::reserve_ratio_bps(0, 100), Ok(0));
    assert_eq!(SettlementReserveMonitor::reserve_ratio_bps(5, 0), Ok(u32::MAX));
    assert_eq!(SettlementReserveMonitor::reserve_ratio_bps(i128::MAX / 10_000, 1), Ok(u32::MAX));
    assert_eq!(
        SettlementReserveMonitor::reserve_ratio_bps(i128::MAX, 1),
        Err(SettlementReserveError::Overflow)
    );
    assert_eq!(
        SettlementReserveMonitor::reserve_ratio_bps(-1, 100),
        Err(SettlementReserveError::InvalidAmount)
    );
    assert_eq!(
        SettlementReserveMonitor::reserve_ratio_bps(1, -100),
        Err(SettlementReserveError::InvalidAmount)
    );
}

#[test]
fn classification_boundaries() {
    let floors = ReserveFloors {
        warning_bps: 11_000,
        critical_bps: 10_000,
    };
    assert_eq!(SettlementReserveMonitor::classify(11_000, &floors), ReserveHealth::Healthy);
    assert_eq!(SettlementReserveMonitor::classify(10_999, &floors), ReserveHealth::Warning);
    assert_eq!(SettlementReserveMonitor::classify(10_000, &floors), ReserveHealth::Warning);
    assert_eq!(SettlementReserveMonitor::classify(9_999, &floors), ReserveHealth::Critical);
}

#[test]
fn healthy_report_emits_no_warning() {
    let ctx = setup();
    let (pos, ev) = report_ev(&ctx, "cbdc_eur", 120, 100);
    let pos = pos.unwrap();
    assert_eq!(pos.health, ReserveHealth::Healthy);
    assert_eq!(pos.ratio_bps, 12_000);
    assert_eq!(count_topic(&ev, EVENT_TOPIC, "warning"), 0);
    assert_eq!(count_topic(&ev, EVENT_TOPIC, "critical"), 0);
}

#[test]
fn below_warning_floor_emits_warning_event() {
    let ctx = setup();
    let (pos, ev) = report_ev(&ctx, "cbdc_eur", 105, 100);
    assert_eq!(pos.unwrap().health, ReserveHealth::Warning);
    assert_eq!(count_topic(&ev, EVENT_TOPIC, "warning"), 1);
    assert_eq!(count_topic(&ev, EVENT_TOPIC, "critical"), 0);
}

#[test]
fn below_critical_floor_emits_critical_event() {
    let ctx = setup();
    let (pos, ev) = report_ev(&ctx, "cbdc_eur", 80, 100);
    assert_eq!(pos.unwrap().health, ReserveHealth::Critical);
    assert_eq!(count_topic(&ev, EVENT_TOPIC, "critical"), 1);
    assert_eq!(count_topic(&ev, EVENT_TOPIC, "warning"), 0);
}

#[test]
fn warning_repeats_while_breach_persists_and_restored_fires_on_recovery() {
    let ctx = setup();
    let (_, ev) = report_ev(&ctx, "pm_payout", 105, 100);
    assert_eq!(count_topic(&ev, EVENT_TOPIC, "warning"), 1);

    // Breach persists: the warning is emitted again.
    let (_, ev) = report_ev(&ctx, "pm_payout", 104, 100);
    assert_eq!(count_topic(&ev, EVENT_TOPIC, "warning"), 1);

    let (pos, ev) = report_ev(&ctx, "pm_payout", 150, 100);
    assert_eq!(pos.unwrap().health, ReserveHealth::Healthy);
    assert_eq!(count_topic(&ev, EVENT_TOPIC, "restored"), 1);

    // Healthy -> healthy does not re-emit `restored`.
    let (_, ev) = report_ev(&ctx, "pm_payout", 160, 100);
    assert_eq!(count_topic(&ev, EVENT_TOPIC, "restored"), 0);
}

#[test]
fn custom_floors_are_per_pool() {
    let ctx = setup();
    ctx.env.ledger().set_timestamp(42);
    let (res, ev) = events_during(&ctx.env, || {
        ctx.env.as_contract(&ctx.contract, || {
            SettlementReserveMonitor::set_floors(
                &ctx.env,
                &ctx.admin,
                Symbol::new(&ctx.env, "strict"),
                15_000,
                12_000,
            )
        })
    });
    res.unwrap();
    assert_eq!(count_topic(&ev, EVENT_TOPIC, "floors"), 1);

    // 130% is a warning for the strict pool but healthy for the default pool.
    assert_eq!(report(&ctx, "strict", 130, 100).unwrap().health, ReserveHealth::Warning);
    assert_eq!(report(&ctx, "default", 130, 100).unwrap().health, ReserveHealth::Healthy);

    let stored = ctx.env.as_contract(&ctx.contract, || {
        SettlementReserveMonitor::get_position(&ctx.env, Symbol::new(&ctx.env, "strict"))
    });
    let stored = stored.unwrap();
    assert_eq!(stored.ratio_bps, 13_000);
    assert_eq!(stored.updated_at, 42);
    assert_eq!(stored.reporter, ctx.reporter);
}

#[test]
fn invalid_floors_rejected() {
    let ctx = setup();
    let set = |w: u32, c: u32| {
        ctx.env.as_contract(&ctx.contract, || {
            SettlementReserveMonitor::set_floors(&ctx.env, &ctx.admin, Symbol::new(&ctx.env, "p"), w, c)
        })
    };
    assert_eq!(set(10_000, 0), Err(SettlementReserveError::InvalidFloors));
    assert_eq!(set(9_000, 10_000), Err(SettlementReserveError::InvalidFloors));
    assert_eq!(set(MAX_FLOOR_BPS + 1, 10_000), Err(SettlementReserveError::InvalidFloors));
    assert!(set(10_000, 10_000).is_ok());
}

#[test]
fn rbac_gates_configuration_and_reporting() {
    let ctx = setup();
    let viewer = Address::generate(&ctx.env);
    let stranger = Address::generate(&ctx.env);
    ctx.env.as_contract(&ctx.contract, || {
        Rbac::set_role(&ctx.env, &ctx.admin, &viewer, Role::Viewer).unwrap();
    });

    // Submitter cannot configure floors.
    let res = ctx.env.as_contract(&ctx.contract, || {
        SettlementReserveMonitor::set_floors(&ctx.env, &ctx.reporter, Symbol::new(&ctx.env, "p"), 11_000, 10_000)
    });
    assert_eq!(res, Err(SettlementReserveError::Unauthorized));

    // Viewer and unknown callers cannot report.
    for who in [&viewer, &stranger] {
        let res = ctx.env.as_contract(&ctx.contract, || {
            SettlementReserveMonitor::report_reserves(&ctx.env, who, Symbol::new(&ctx.env, "p"), 1, 1)
        });
        assert_eq!(res, Err(SettlementReserveError::Unauthorized));
    }

    // Suspended reporter is rejected with a distinct code.
    ctx.env.as_contract(&ctx.contract, || {
        Rbac::suspend(&ctx.env, &ctx.admin, &ctx.reporter).unwrap();
    });
    assert_eq!(report(&ctx, "p", 1, 1), Err(SettlementReserveError::Suspended));
}
