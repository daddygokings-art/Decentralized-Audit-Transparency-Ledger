//! # Settlement Reserve Monitoring
//!
//! Tracks the reserve coverage of settlement pools (e.g. CBDC cross-ledger
//! settlement, prediction-market payouts) and emits warning events whenever a
//! pool's reserve ratio falls below its configured safety floors.
//!
//! ```text
//! ratio_bps = reserves * 10_000 / liabilities      (u32::MAX when liabilities == 0)
//!
//!   ratio_bps >= warning_bps                 → Healthy
//!   critical_bps <= ratio_bps < warning_bps  → Warning   (emits `settle_rsv/warning`)
//!   ratio_bps <  critical_bps                → Critical  (emits `settle_rsv/critical`)
//! ```
//!
//! A `settle_rsv/restored` event is emitted when a pool returns to `Healthy`
//! from `Warning` or `Critical`. Warning and critical events fire on *every*
//! below-floor report (not just the first crossing), so off-chain monitors see
//! a continuous signal for the whole time a breach lasts.
//!
//! ## Events
//!
//! | Topics | Data |
//! |--------|------|
//! | `("settle_rsv", "warning")`  | `(pool, ratio_bps, warning_bps, reserves, liabilities, timestamp)` |
//! | `("settle_rsv", "critical")` | `(pool, ratio_bps, critical_bps, reserves, liabilities, timestamp)` |
//! | `("settle_rsv", "restored")` | `(pool, ratio_bps, previous_health, timestamp)` |
//! | `("settle_rsv", "floors")`   | `(pool, caller, warning_bps, critical_bps)` |
//!
//! ## Access control
//!
//! Floors are configured by `Admin`; positions are reported by `Submitter` or
//! higher (see [`crate::rbac`]). Suspended DIDs are rejected.
//!
//! ## Metering
//!
//! A report costs two persistent reads (floors, previous position), one
//! persistent write and at most one event. The ratio uses checked `i128`
//! arithmetic with no loops, so cost is constant per report.

use crate::rbac::{RbacError, RbacManager, Role};
use soroban_sdk::{contracterror, contracttype, Address, Env, Symbol};

/// Basis-points denominator (100% = 10_000 bps).
pub const BPS_DENOMINATOR: i128 = 10_000;
/// Default warning floor when a pool has none configured (110%).
pub const DEFAULT_WARNING_BPS: u32 = 11_000;
/// Default critical floor when a pool has none configured (100%).
pub const DEFAULT_CRITICAL_BPS: u32 = 10_000;
/// Largest floor that may be configured (10_000%).
pub const MAX_FLOOR_BPS: u32 = 1_000_000;

/// Event topic shared by all settlement reserve events.
pub const EVENT_TOPIC: &str = "settle_rsv";

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum SettlementReserveError {
    /// Caller lacks the RBAC role required for this operation.
    Unauthorized = 7201,
    /// Caller's DID is suspended.
    Suspended = 7202,
    /// Floors must satisfy `0 < critical <= warning <= MAX_FLOOR_BPS`.
    InvalidFloors = 7203,
    /// Reserves and liabilities must be non-negative.
    InvalidAmount = 7204,
    /// Arithmetic overflow while computing the ratio.
    Overflow = 7205,
}

impl From<RbacError> for SettlementReserveError {
    fn from(err: RbacError) -> Self {
        match err {
            RbacError::SubmitterSuspended | RbacError::Suspended => Self::Suspended,
            _ => Self::Unauthorized,
        }
    }
}

/// Health classification of a settlement pool.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ReserveHealth {
    Healthy = 0,
    Warning = 1,
    Critical = 2,
}

/// Safety floors for one settlement pool, in basis points.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReserveFloors {
    pub warning_bps: u32,
    pub critical_bps: u32,
}

/// Latest reported reserve position of a settlement pool.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReservePosition {
    pub pool: Symbol,
    pub reserves: i128,
    pub liabilities: i128,
    pub ratio_bps: u32,
    pub health: ReserveHealth,
    pub reporter: Address,
    pub updated_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SettlementReserveKey {
    Floors(Symbol),
    Position(Symbol),
}

pub struct SettlementReserveMonitor;

impl SettlementReserveMonitor {
    /// Configures the safety floors for `pool`. Requires `Admin`.
    pub fn set_floors(
        env: &Env,
        caller: &Address,
        pool: Symbol,
        warning_bps: u32,
        critical_bps: u32,
    ) -> Result<ReserveFloors, SettlementReserveError> {
        RbacManager::require_role_precedence(env, caller, Role::Admin)?;

        if critical_bps == 0 || critical_bps > warning_bps || warning_bps > MAX_FLOOR_BPS {
            return Err(SettlementReserveError::InvalidFloors);
        }

        let floors = ReserveFloors {
            warning_bps,
            critical_bps,
        };
        env.storage()
            .persistent()
            .set(&SettlementReserveKey::Floors(pool.clone()), &floors);

        env.events().publish(
            (Symbol::new(env, EVENT_TOPIC), Symbol::new(env, "floors")),
            (pool, caller.clone(), warning_bps, critical_bps),
        );
        Ok(floors)
    }

    /// Returns the floors for `pool`, falling back to the defaults.
    pub fn get_floors(env: &Env, pool: Symbol) -> ReserveFloors {
        env.storage()
            .persistent()
            .get(&SettlementReserveKey::Floors(pool))
            .unwrap_or(ReserveFloors {
                warning_bps: DEFAULT_WARNING_BPS,
                critical_bps: DEFAULT_CRITICAL_BPS,
            })
    }

    /// Returns the last reported position for `pool`, if any.
    pub fn get_position(env: &Env, pool: Symbol) -> Option<ReservePosition> {
        env.storage()
            .persistent()
            .get(&SettlementReserveKey::Position(pool))
    }

    /// Computes `reserves / liabilities` in basis points.
    ///
    /// Returns `u32::MAX` when there are no liabilities (fully covered) and
    /// saturates at `u32::MAX` for very large ratios.
    pub fn reserve_ratio_bps(reserves: i128, liabilities: i128) -> Result<u32, SettlementReserveError> {
        if reserves < 0 || liabilities < 0 {
            return Err(SettlementReserveError::InvalidAmount);
        }
        if liabilities == 0 {
            return Ok(u32::MAX);
        }
        let scaled = reserves
            .checked_mul(BPS_DENOMINATOR)
            .ok_or(SettlementReserveError::Overflow)?;
        let ratio = scaled / liabilities;
        Ok(if ratio > u32::MAX as i128 { u32::MAX } else { ratio as u32 })
    }

    /// Classifies a ratio against a pool's floors.
    pub fn classify(ratio_bps: u32, floors: &ReserveFloors) -> ReserveHealth {
        if ratio_bps < floors.critical_bps {
            ReserveHealth::Critical
        } else if ratio_bps < floors.warning_bps {
            ReserveHealth::Warning
        } else {
            ReserveHealth::Healthy
        }
    }

    /// Records a reserve observation for `pool` and emits warning events when
    /// the ratio is below a safety floor. Requires `Submitter` or higher.
    pub fn report_reserves(
        env: &Env,
        reporter: &Address,
        pool: Symbol,
        reserves: i128,
        liabilities: i128,
    ) -> Result<ReservePosition, SettlementReserveError> {
        RbacManager::require_role_precedence(env, reporter, Role::Submitter)?;

        let ratio_bps = Self::reserve_ratio_bps(reserves, liabilities)?;
        let floors = Self::get_floors(env, pool.clone());
        let health = Self::classify(ratio_bps, &floors);
        let previous = Self::get_position(env, pool.clone()).map(|p| p.health);
        let now = env.ledger().timestamp();

        let position = ReservePosition {
            pool: pool.clone(),
            reserves,
            liabilities,
            ratio_bps,
            health,
            reporter: reporter.clone(),
            updated_at: now,
        };
        env.storage()
            .persistent()
            .set(&SettlementReserveKey::Position(pool.clone()), &position);

        let topic = Symbol::new(env, EVENT_TOPIC);
        match health {
            ReserveHealth::Critical => env.events().publish(
                (topic, Symbol::new(env, "critical")),
                (pool, ratio_bps, floors.critical_bps, reserves, liabilities, now),
            ),
            ReserveHealth::Warning => env.events().publish(
                (topic, Symbol::new(env, "warning")),
                (pool, ratio_bps, floors.warning_bps, reserves, liabilities, now),
            ),
            ReserveHealth::Healthy => {
                if let Some(prev) = previous {
                    if prev != ReserveHealth::Healthy {
                        env.events().publish(
                            (topic, Symbol::new(env, "restored")),
                            (pool, ratio_bps, prev as u32, now),
                        );
                    }
                }
            }
        }

        Ok(position)
    }
}
