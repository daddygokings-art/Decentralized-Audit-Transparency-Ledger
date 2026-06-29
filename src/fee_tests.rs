use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{symbol_short, Bytes, Env, Vec};

/// Soroban test-mode budget limits used as acceptance thresholds.
/// Values sourced from Stellar testnet fee schedule (Protocol 21).
/// CPU instructions: 100_000_000 per transaction.
/// Memory bytes:      41_943_040 per transaction.
const MAX_CPU_INSNS: u64 = 100_000_000;
const MAX_MEM_BYTES: u64 = 41_943_040;

fn setup() -> (Env, Address, AuditLedgerClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let owner = Address::generate(&env);
    let cid = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &cid);
    client.initialize(&owner, &1_000_000);
    (env, owner, client)
}

fn budget(env: &Env) -> (u64, u64) {
    let cpu = env.cost_estimate().budget().cpu_instruction_count();
    let mem = env.cost_estimate().budget().memory_bytes_used();
    (cpu, mem)
}

fn reset(env: &Env) {
    env.cost_estimate().budget().reset_default();
}

// ── initialize ────────────────────────────────────────────────────────────────

#[test]
fn fee_initialize() {
    let env = Env::default();
    env.mock_all_auths();
    let owner = Address::generate(&env);
    let cid = env.register(AuditLedger, ());
    let client = AuditLedgerClient::new(&env, &cid);

    reset(&env);
    client.initialize(&owner, &100_000);
    let (cpu, mem) = budget(&env);

    assert!(cpu < MAX_CPU_INSNS, "initialize cpu {cpu} exceeds limit");
    assert!(mem < MAX_MEM_BYTES, "initialize mem {mem} exceeds limit");
}

// ── log_event — varying metadata sizes ───────────────────────────────────────

#[test]
fn fee_log_event_small_metadata_10b() {
    let (env, _, client) = setup();
    let submitter = Address::generate(&env);
    let meta = Bytes::from_slice(&env, &[0u8; 10]);

    reset(&env);
    client.log_event(&submitter, &symbol_short!("payment"), &meta);
    let (cpu, mem) = budget(&env);

    assert!(
        cpu < MAX_CPU_INSNS,
        "log_event(10B) cpu {cpu} exceeds limit"
    );
    assert!(
        mem < MAX_MEM_BYTES,
        "log_event(10B) mem {mem} exceeds limit"
    );
}

#[test]
fn fee_log_event_medium_metadata_100b() {
    let (env, _, client) = setup();
    let submitter = Address::generate(&env);
    let meta = Bytes::from_slice(&env, &[0u8; 100]);

    reset(&env);
    client.log_event(&submitter, &symbol_short!("payment"), &meta);
    let (cpu, mem) = budget(&env);

    assert!(
        cpu < MAX_CPU_INSNS,
        "log_event(100B) cpu {cpu} exceeds limit"
    );
    assert!(
        mem < MAX_MEM_BYTES,
        "log_event(100B) mem {mem} exceeds limit"
    );
}

#[test]
fn fee_log_event_large_metadata_1kb() {
    let (env, _, client) = setup();
    let submitter = Address::generate(&env);
    let meta = Bytes::from_slice(&env, &[0u8; 1024]);

    reset(&env);
    client.log_event(&submitter, &symbol_short!("payment"), &meta);
    let (cpu, mem) = budget(&env);

    assert!(
        cpu < MAX_CPU_INSNS,
        "log_event(1KB) cpu {cpu} exceeds limit"
    );
    assert!(
        mem < MAX_MEM_BYTES,
        "log_event(1KB) mem {mem} exceeds limit"
    );
}

// ── log_events batch vs single comparison ────────────────────────────────────

#[test]
fn fee_log_events_batch_10_vs_single() {
    let (env, _, client) = setup();
    let submitter = Address::generate(&env);
    let meta = Bytes::from_slice(&env, &[0u8; 64]);
    let event_type = symbol_short!("transfer");

    // Measure 10 individual log_event calls
    let mut single_cpu_total: u64 = 0;
    let mut single_mem_total: u64 = 0;
    for _ in 0..10 {
        reset(&env);
        client.log_event(&submitter, &event_type, &meta);
        let (c, m) = budget(&env);
        single_cpu_total += c;
        single_mem_total += m;
    }

    // Measure one log_events batch of 10
    let mut batch: Vec<(Address, Symbol, Bytes)> = Vec::new(&env);
    for _ in 0..10u32 {
        batch.push_back((submitter.clone(), event_type.clone(), meta.clone()));
    }

    reset(&env);
    client.log_events(&batch);
    let (batch_cpu, batch_mem) = budget(&env);

    // Batch must not exceed the per-transaction limit
    assert!(
        batch_cpu < MAX_CPU_INSNS,
        "batch cpu {batch_cpu} exceeds limit"
    );
    assert!(
        batch_mem < MAX_MEM_BYTES,
        "batch mem {batch_mem} exceeds limit"
    );

    // Batch should be cheaper (or at most equal) than the sum of singles in CPU
    // (this is a regression guard — if batch becomes more expensive, investigate)
    assert!(
        batch_cpu <= single_cpu_total,
        "batch cpu ({batch_cpu}) > sum of singles ({single_cpu_total}): batch is not cheaper"
    );
    let _ = single_mem_total; // mem comparison is informational; not asserted
}

// ── read operations ───────────────────────────────────────────────────────────

#[test]
fn fee_get_event() {
    let (env, _, client) = setup();
    let submitter = Address::generate(&env);
    let id = client.log_event(
        &submitter,
        &symbol_short!("payment"),
        &Bytes::from_slice(&env, b"test"),
    );

    reset(&env);
    let _ = client.get_event(&id);
    let (cpu, mem) = budget(&env);

    assert!(cpu < MAX_CPU_INSNS, "get_event cpu {cpu} exceeds limit");
    assert!(mem < MAX_MEM_BYTES, "get_event mem {mem} exceeds limit");
}

#[test]
fn fee_get_event_by_type() {
    let (env, _, client) = setup();
    let submitter = Address::generate(&env);
    let event_type = symbol_short!("payment");
    client.log_event(&submitter, &event_type, &Bytes::from_slice(&env, b"test"));

    reset(&env);
    let _ = client.get_event_by_type(&event_type, &0);
    let (cpu, mem) = budget(&env);

    assert!(
        cpu < MAX_CPU_INSNS,
        "get_event_by_type cpu {cpu} exceeds limit"
    );
    assert!(
        mem < MAX_MEM_BYTES,
        "get_event_by_type mem {mem} exceeds limit"
    );
}

// ── governance functions ──────────────────────────────────────────────────────

#[test]
fn fee_set_global_max_logs() {
    let (env, owner, client) = setup();

    reset(&env);
    client.set_global_max_logs(&owner, &500_000);
    let (cpu, mem) = budget(&env);

    assert!(
        cpu < MAX_CPU_INSNS,
        "set_global_max_logs cpu {cpu} exceeds limit"
    );
    assert!(
        mem < MAX_MEM_BYTES,
        "set_global_max_logs mem {mem} exceeds limit"
    );
}

#[test]
fn fee_set_event_max_logs() {
    let (env, owner, client) = setup();

    reset(&env);
    client.set_event_max_logs(&owner, &symbol_short!("payment"), &100);
    let (cpu, mem) = budget(&env);

    assert!(
        cpu < MAX_CPU_INSNS,
        "set_event_max_logs cpu {cpu} exceeds limit"
    );
    assert!(
        mem < MAX_MEM_BYTES,
        "set_event_max_logs mem {mem} exceeds limit"
    );
}

#[test]
fn fee_remove_event_cap() {
    let (env, owner, client) = setup();
    client.set_event_max_logs(&owner, &symbol_short!("payment"), &100);

    reset(&env);
    client.remove_event_cap(&owner, &symbol_short!("payment"));
    let (cpu, mem) = budget(&env);

    assert!(
        cpu < MAX_CPU_INSNS,
        "remove_event_cap cpu {cpu} exceeds limit"
    );
    assert!(
        mem < MAX_MEM_BYTES,
        "remove_event_cap mem {mem} exceeds limit"
    );
}

#[test]
fn fee_transfer_ownership() {
    let (env, owner, client) = setup();
    let new_owner = Address::generate(&env);

    reset(&env);
    client.transfer_ownership(&owner, &new_owner);
    let (cpu, mem) = budget(&env);

    assert!(
        cpu < MAX_CPU_INSNS,
        "transfer_ownership cpu {cpu} exceeds limit"
    );
    assert!(
        mem < MAX_MEM_BYTES,
        "transfer_ownership mem {mem} exceeds limit"
    );
}
