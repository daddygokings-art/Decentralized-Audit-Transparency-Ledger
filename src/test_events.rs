//! Shared test helpers for asserting on emitted contract events.
//!
//! Depending on the SDK version, `env.events().all()` either accumulates every
//! event or only holds those of the most recent frame. [`events_during`] hides
//! that difference by diffing the event log around a closure.

use alloc::vec::Vec as StdVec;
use soroban_sdk::xdr::{ContractEvent, ContractEventBody, ScSymbol, ScVal};
use soroban_sdk::Env;

fn snapshot(env: &Env) -> StdVec<ContractEvent> {
    env.events().all().events().to_vec()
}

/// Runs `f` and returns its result together with the events it emitted.
pub fn events_during<T>(env: &Env, f: impl FnOnce() -> T) -> (T, StdVec<ContractEvent>) {
    let before = snapshot(env);
    let out = f();
    let after = snapshot(env);
    let emitted = if after.len() >= before.len() && after[..before.len()] == before[..] {
        after[before.len()..].to_vec()
    } else {
        after
    };
    (out, emitted)
}

/// Symbol `ScVal` for topic comparisons.
pub fn sym(s: &str) -> ScVal {
    ScVal::Symbol(ScSymbol(s.try_into().unwrap()))
}

/// Number of events whose first two topics are `(t0, t1)`.
pub fn count_topic(events: &[ContractEvent], t0: &str, t1: &str) -> usize {
    let (a, b) = (sym(t0), sym(t1));
    events
        .iter()
        .filter(|e| match &e.body {
            ContractEventBody::V0(v) => v.topics.len() >= 2 && v.topics[0] == a && v.topics[1] == b,
        })
        .count()
}

/// Number of events whose first topic is `t0`.
pub fn count_first_topic(events: &[ContractEvent], t0: &str) -> usize {
    let a = sym(t0);
    events
        .iter()
        .filter(|e| match &e.body {
            ContractEventBody::V0(v) => !v.topics.is_empty() && v.topics[0] == a,
        })
        .count()
}
