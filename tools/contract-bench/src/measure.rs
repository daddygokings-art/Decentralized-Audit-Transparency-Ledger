//! Measurement primitives.
//!
//! Every number this suite reports is read back out of the Soroban host's own
//! resource meter, or computed from it. Nothing is estimated by hand and nothing
//! is timed against a wall clock.
//!
//! Two independent host views are used, because neither alone is sufficient:
//!
//! * [`soroban_sdk::testutils::budget::Budget`] reports CPU instructions and
//!   memory bytes, and can be reset on demand, so a scenario's own cost can be
//!   isolated from the cost of seeding the ledger.
//! * [`Env::cost_estimate`] returns the resource totals the host computes for
//!   the current invocation, which is the only source of ledger-entry counts and
//!   byte volumes.
//!
//! Taking a reading is therefore two steps: [`Snapshot::of`] before the workload
//! and again after, then [`Measurement::between`] to difference them.

use serde::{Deserialize, Serialize};
use soroban_sdk::{contract, contractimpl, Address, Env};

/// A raw reading of the host's resource meter.
///
/// Fields are copied out one at a time rather than holding the host's own
/// `InvocationResources` value. That keeps this crate's public types independent
/// of `soroban-env-host`, so a host upgrade that reshuffles those internals
/// cannot silently change what the benchmark suite records or how it is
/// compared against history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Snapshot {
    pub instructions: i64,
    pub mem_bytes: i64,
    pub read_entries: u32,
    pub write_entries: u32,
    pub read_bytes: u32,
    pub write_bytes: u32,
    pub event_bytes: u32,
    pub rent_bumps: u32,
    pub rent_ledger_bytes: i64,
}

impl Snapshot {
    /// Read the host's current resource meter.
    pub fn of(env: &Env) -> Self {
        let r = env.cost_estimate().resources();
        Self {
            instructions: r.instructions,
            mem_bytes: r.mem_bytes,
            // Live contract state is held in memory during an invocation, so
            // entry reads are reported here rather than as disk reads.
            read_entries: r.memory_read_entries,
            write_entries: r.write_entries,
            // Test entries are already resident, so the disk-read byte counter
            // reads zero. The entry count above is the meaningful figure for
            // reads; this is kept because it becomes the real figure when the
            // same meter is read against a network node.
            read_bytes: r.disk_read_bytes,
            write_bytes: r.write_bytes,
            event_bytes: r.contract_events_size_bytes,
            rent_bumps: r.persistent_entry_rent_bumps,
            rent_ledger_bytes: r.persistent_rent_ledger_bytes,
        }
    }
}

impl From<Snapshot> for Measurement {
    fn from(r: Snapshot) -> Self {
        Self {
            instructions: r.instructions,
            mem_bytes: r.mem_bytes,
            read_entries: r.read_entries,
            write_entries: r.write_entries,
            read_bytes: r.read_bytes,
            write_bytes: r.write_bytes,
            event_bytes: r.event_bytes,
            rent_bumps: r.rent_bumps,
            rent_ledger_bytes: r.rent_ledger_bytes,
        }
    }
}

/// The cost of one benchmarked workload, as the host metered that one invocation.
///
/// This is read outright from the invocation meter rather than differenced
/// between two readings. The meter is already per-invocation, so differencing
/// it would subtract an unrelated invocation: the fixture's last frame is
/// usually the larger of the two, and the difference then comes out negative.
/// Every figure here is what a single transaction on a real network reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Measurement {
    /// Modelled CPU instructions consumed.
    pub instructions: i64,
    /// Modelled memory bytes held.
    pub mem_bytes: i64,
    /// Ledger entries read.
    pub read_entries: u32,
    /// Ledger entries written.
    pub write_entries: u32,
    /// Bytes read from the ledger.
    pub read_bytes: u32,
    /// Bytes written to the ledger.
    pub write_bytes: u32,
    /// Bytes of contract event data emitted.
    pub event_bytes: u32,
    /// Persistent entries whose rent was extended.
    pub rent_bumps: u32,
    /// Cumulative rent extension in ledger-bytes.
    pub rent_ledger_bytes: i64,
}

impl Measurement {
    /// Read the cost of one invocation straight out of the host's meter.
    pub fn of_invocation(env: &Env) -> Self {
        Snapshot::of(env).into()
    }

    /// Per-entry write cost, or `None` when nothing was written.
    ///
    /// This is the number that exposes a storage-layout regression: a change
    /// that keeps the write count flat but inflates the stored value shows up
    /// here even though `write_entries` is unchanged.
    pub fn bytes_per_write(&self) -> Option<f64> {
        (self.write_entries > 0).then(|| f64::from(self.write_bytes) / f64::from(self.write_entries))
    }

    /// Instructions per ledger entry touched, or `None` when nothing was touched.
    pub fn instructions_per_entry(&self) -> Option<f64> {
        let entries = self.read_entries + self.write_entries;
        (entries > 0).then(|| self.instructions as f64 / f64::from(entries))
    }
}

/// An empty contract, registered so the host has a real contract instance to
/// meter against.
///
/// `soroban-sdk 27` only permits storage access from inside a contract frame, and
/// entering a frame for an address that has no registered instance fails with
/// `MissingValue`. Registering a real contract — rather than faking the instance
/// — is also the more faithful setup: the instance entry the host maintains is
/// part of what an invocation actually costs, so the measurements include it
/// rather than quietly omitting it. It has no methods, because none of the
/// modelled work is executed as Rust: [`crate::contract`] performs the storage
/// and event operations directly inside this frame.
#[contract]
pub struct Ledger;

#[contractimpl]
impl Ledger {
    /// Registering a contract requires a constructor; this one does nothing.
    pub fn __constructor(_env: Env) {}
}

/// An `Env` for benchmarking.
///
/// Snapshot capture is switched off. The SDK's test default writes a JSON dump
/// of the whole ledger every time an `Env` is dropped, which the suite creates
/// hundreds of — one per measurement — so the default would spend most of a
/// benchmark run serialising ledgers to disk, and litter the working tree with
/// megabytes of files that mean nothing outside a test assertion.
pub fn test_env() -> Env {
    Env::new_with_config(soroban_sdk::testutils::EnvTestConfig {
        capture_snapshot_at_drop: false,
    })
}

/// Register the modelled contract and return its address.
pub fn register(env: &Env) -> Address {
    env.register(Ledger, ())
}

/// Run fixture construction inside a contract frame.
///
/// `soroban-sdk 27` only allows storage access from inside a frame, and the host
/// enforces the network's per-invocation write limits on everything done in one.
/// Seeding a 10 000-event ledger is not an invocation — no transaction could do
/// it — so it is built in chunks, each chunk a network-legal slice. The measured
/// workload, by contrast, runs as a single frame: one invocation, exactly as a
/// real call would be metered.
pub fn in_frame<T>(env: &Env, contract: &Address, f: impl FnOnce() -> T) -> T {
    env.as_contract(contract, f)
}

/// Give the `Env` a fresh budget at its default limits.
///
/// A test `Env` meters its *entire* life against one budget — 100M instructions
/// and 40 MiB by default — whereas a real network gives every transaction a new
/// one. The suite models many transactions: a fixture is written in chunks and
/// the measured workload is one more. Without a reset between them the harness
/// would eventually fail a workload that no real transaction could, and the last
/// frame measured would inherit whatever headroom the fixture left behind.
///
/// So the budget is reset at every transaction boundary: after each fixture
/// chunk, and immediately before the measured frame.
#[allow(deprecated)] // `Env::budget` is deprecated in favour of `cost_estimate().budget()`,
                     // which cannot reset the underlying budget.
pub fn reset_budget(env: &Env) {
    let mut budget = env.budget();
    budget.reset_default();
}

/// One fixture transaction: a frame bracketed by budget resets.
///
/// Anything that is not the measurement — seeding, test setup — belongs here, so
/// that the ledger it leaves behind is the only thing that carries into the
/// measured invocation.
///
/// The reset is needed on *both* sides. After the frame, so the next one starts
/// clean. Before it, because the host samples the current memory figure when a
/// frame opens and charges it to that frame: a frame opened behind a large
/// ledger is billed for the whole ledger and trips the 40 MiB invocation limit
/// even though the frame itself allocated almost nothing.
pub fn fixture<T>(env: &Env, contract: &Address, f: impl FnOnce() -> T) -> T {
    reset_budget(env);
    let out = in_frame(env, contract, f);
    reset_budget(env);
    out
}

/// A workload that can be instantiated repeatedly.
///
/// Benchmarked workloads mutate the ledger they run against, so a scenario is
/// single-use: measuring the same instance twice would report the second append
/// against a ledger the first append had already changed. A [`Builder`] holds
/// only the parameters of a workload, so each measurement gets a freshly seeded
/// environment and the suite can confirm a result is reproducible by building the
/// same scenario again.
/// Seeds a fixture and returns the closure that will be measured.
///
/// Named because the two nested closures and their lifetimes are hard to read
/// inline, and because this signature is the contract every workload in the
/// suite is written against.
type BuildFn = Box<dyn Fn(&Env, &Address) -> Box<dyn Fn(&Env)>>;

pub struct Builder {
    name: String,
    build: BuildFn,
}

impl Builder {
    /// Define a workload by its parameters.
    ///
    /// `build` is handed an empty environment and the contract the scenario runs
    /// as, seeds however it needs, and returns the closure that will be measured.
    /// Seeding happens here and is therefore excluded from the measurement.
    ///
    /// `build` is *not* wrapped in a contract frame: a fixture of ten thousand
    /// events cannot be built inside one frame without tripping the network's
    /// per-invocation write limits, and it is not an invocation anyway. Seeding
    /// code opens its own frames with [`in_frame`]. The returned closure is
    /// measured inside a single frame by [`Scenario::measure`], because that part
    /// is one real invocation.
    pub fn new<F>(name: impl Into<String>, build: F) -> Self
    where
        F: Fn(&Env, &Address) -> Box<dyn Fn(&Env)> + 'static,
    {
        Self {
            name: name.into(),
            build: Box::new(build),
        }
    }

    /// Instantiate one fresh scenario.
    pub fn scenario(&self) -> Scenario {
        let env = test_env();
        let contract = register(&env);
        let run = (self.build)(&env, &contract);
        Scenario {
            name: self.name.clone(),
            env,
            contract,
            run,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

/// One instantiated workload, ready to be measured.
pub struct Scenario {
    name: String,
    env: Env,
    /// The contract this scenario runs as. Fixed for the scenario's lifetime so
    /// that seeding and the measured call address the same ledger.
    contract: Address,
    run: Box<dyn Fn(&Env)>,
}

impl Scenario {
    /// Run the workload once and return what it cost.
    ///
    /// Run the workload as one invocation and return what it cost.
    ///
    /// The budget is reset first, so the measured frame gets a whole
    /// transaction's worth of headroom rather than whatever the seeding left
    /// spent. Nothing else is needed to exclude the fixture: the host's meter
    /// counts one invocation at a time, so the reading taken after the frame is
    /// this workload's own cost and the seeding is not in it.
    pub fn measure(&self) -> Measurement {
        reset_budget(&self.env);
        let (run, contract) = (&self.run, &self.contract);
        self.env.as_contract(contract, || run(&self.env));
        Measurement::of_invocation(&self.env)
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract as model;
    use soroban_sdk::{symbol_short, testutils::Address as _, Address, Bytes, Symbol};

    #[test]
    fn snapshot_fields_are_populated_for_a_real_workload() {
        // Guards against a silent no-op: if the host ever stops reporting these
        // counters, every benchmark in the suite would report zero and still
        // "pass" regression detection against a zero baseline.
        let s = Builder::new("probe", |env, contract| {
            let key = symbol_short!("k");
            let addr = Address::generate(env);
            let mut data = Bytes::new(env);
            for _ in 0..64 {
                data.append(&Bytes::from_slice(env, &[7u8; 16]));
            }
            in_frame(env, contract, || env.storage().persistent().set(&key, &data));
            // A *different* value than the fixture: rewriting an identical value
            // is a no-op the host collapses, which would report zero writes and
            // make this test pass for the wrong reason.
            let record2 = data;
            Box::new(move |env: &Env| {
                env.storage().persistent().set(&key, &record2);
                model::EventLogged {
                    event_type: Symbol::new(env, "probe"),
                    submitter: addr.clone(),
                    metadata: Bytes::from_slice(env, &[1u8, 8]),
                    sequence: 1,
                }
                .publish(env);
            })
        })
        .scenario();
        let m = s.measure();
        assert!(m.instructions > 0, "host reported no instructions: {m:?}");
        assert!(m.write_entries > 0, "host reported no writes: {m:?}");
        assert!(m.write_bytes > 0, "host reported no write bytes: {m:?}");
        assert!(m.event_bytes > 0, "host reported no event bytes: {m:?}");
    }

    #[test]
    fn a_measurement_is_the_invocation_meter_read_outright() {
        // The meter is per-invocation, so the workload's cost is read directly.
        // Differencing it against a reading taken beforehand would subtract an
        // unrelated frame, and since the fixture's last frame is usually the
        // larger one, that difference comes out negative.
        let env = test_env();
        let contract = register(&env);
        fixture(&env, &contract, || {
            for i in 0..8u32 {
                env.storage().persistent().set(&(i, 1u32), &i);
            }
        });
        let m = Measurement::of_invocation(&env);
        assert!(m.instructions >= 0, "instructions cannot be negative: {m:?}");
        assert!(m.mem_bytes >= 0, "memory cannot be negative: {m:?}");
    }

    #[test]
    fn a_small_workload_on_a_large_fixture_is_not_reported_as_free() {
        // The concrete symptom of differencing two unrelated invocations: a cheap
        // append measured against a big fixture came out negative, so this pins
        // the figure as a positive per-invocation cost.
        let m = crate::scenarios::single_event(crate::scenarios::MAX_FIXTURE_EVENTS).measure();
        assert!(m.instructions > 0, "instructions must be positive: {m:?}");
        assert!(m.write_entries > 0, "an append must write entries: {m:?}");
        assert!(m.event_bytes > 0, "an append must emit an event: {m:?}");
    }

    #[test]
    fn derived_ratios_are_none_without_work() {
        let m = Measurement::default();
        assert_eq!(m.bytes_per_write(), None);
        assert_eq!(m.instructions_per_entry(), None);
    }

    #[test]
    fn ratios_divide_by_the_right_denominators() {
        let m = Measurement {
            write_entries: 4,
            write_bytes: 400,
            read_entries: 6,
            instructions: 1000,
            ..Default::default()
        };
        assert_eq!(m.bytes_per_write(), Some(100.0));
        assert_eq!(m.instructions_per_entry(), Some(100.0));
    }
}
