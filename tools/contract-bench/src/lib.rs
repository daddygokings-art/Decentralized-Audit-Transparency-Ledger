//! Benchmark suite for the AuditLedger contract.
//!
//! # What this measures, and what it does not
//!
//! Every figure this suite reports is a reading from a real Soroban host's
//! resource meter, taken while the host executed the contract's actual storage,
//! event and rent operations. Nothing is timed against a wall clock and nothing
//! is estimated by hand.
//!
//! What is not measured is the contract's own code. The `audit-ledger` crate does
//! not compile, so it cannot be linked, called, or profiled; the operations the
//! contract performs are transcribed from its source into [`contract`] instead.
//! The figures are therefore a **lower bound** on production cost, biased low by
//! exactly the amount the contract's arithmetic and VM execution would add.
//! `suite::MODEL` carries this statement inside every result file so a number
//! cannot be quoted later without it.
//!
//! That bias does not defeat the purpose. For a ledger contract the host
//! operations dominate — an append writes several XDR-encoded entries, extends
//! rent on each, and rewrites two index arrays that grow with the ledger — and
//! those are the parts a change usually makes worse. The suite is built to catch
//! exactly that, and `docs/performance/benchmark-methodology.md` states the
//! limitation, the bias direction, and what would have to change for the numbers
//! to become measurements of the deployed contract.
//!
//! # Why a separate crate
//!
//! It shares no compilation unit with the contract, so it keeps measuring while
//! the contract does not build, and a refactor of the contract cannot silently
//! change what is being measured.
//!
//! # Why not wall-clock time
//!
//! Run-to-run variance on a shared CI runner routinely exceeds 5%, which is the
//! threshold this suite is required to enforce. A wall-clock gate at 5% would fail
//! at random and get muted. Host-metered quantities are deterministic — the tests
//! assert two suite runs serialise byte-identically — so 5% here is a real signal.
//!
//! # Usage
//!
//! ```text
//! audit-ledger-bench run        measure everything, compare to the baseline, keep history
//! audit-ledger-bench report     print a Markdown report for the last run
//! audit-ledger-bench history    chart a metric across retained runs
//! audit-ledger-bench promote    accept the latest run as the new baseline
//! audit-ledger-bench wasm       report WASM size and budget status
//! audit-ledger-bench export     write Prometheus exposition text
//! ```

pub mod contract;
pub mod functions;
pub mod history;
pub mod measure;
pub mod regress;
pub mod report;
pub mod scenarios;
pub mod suite;
pub mod version;
pub mod wasm;

pub use suite::{BenchResult, BenchSuite, MODEL};
