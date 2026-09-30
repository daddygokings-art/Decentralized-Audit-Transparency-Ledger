//! `cargo bench --bench storage` — each host operation in isolation.
//!
//! The scenario benchmarks measure workflows; these measure the primitives those
//! workflows are built from, so that a regression can be attributed to "the packed
//! index rewrite got more expensive" rather than to "log_event got more expensive".

use contract_bench::suite::{storage_cases, write_group_json, BenchResult};

fn main() {
    let cases = storage_cases();
    println!("storage primitives: {} cases", cases.len());
    println!(
        "{:<28} {:>10} {:>7} {:>7} {:>10} {:>10}",
        "primitive", "instr", "reads", "writes", "write_B", "events_B"
    );

    let mut results = Vec::new();
    for case in &cases {
        let m = case.measure();
        println!(
            "{:<28} {:>10} {:>7} {:>7} {:>10} {:>10}",
            case.name, m.instructions, m.read_entries, m.write_entries, m.write_bytes, m.event_bytes
        );
        results.push(BenchResult::measured(
            &case.id,
            &case.group,
            &case.name,
            case.params(),
            m,
        ));
    }

    // A read primitive must read and not write; a write primitive the reverse.
    for r in &results {
        let (reads, writes) = (
            r.metric("read_entries").unwrap_or(0),
            r.metric("write_entries").unwrap_or(0),
        );
        match r.name.as_str() {
            "read_persistent" | "has_persistent" => {
                assert!(reads > 0, "{} must read", r.name);
                assert_eq!(writes, 0, "{} must not write", r.name);
            }
            "write_instance_scalar" | "write_persistent_bytes32" | "rewrite_packed_index" => {
                assert!(writes > 0, "{} must write", r.name);
            }
            "emit_event" => {
                assert!(
                    r.metric("event_bytes").unwrap_or(0) > 0,
                    "emitting must produce event bytes"
                );
                assert_eq!(writes, 0, "publishing an event writes no ledger entry");
            }
            "rent_extension" => {
                assert!(
                    r.metric("rent_bumps").unwrap_or(0) > 0,
                    "a rent extension must be metered as a rent bump"
                );
            }
            _ => {}
        }
    }

    // The packed index rewrite is the primitive that scales with ledger size, so
    // it must write more than a scalar write even at equal instruction counts.
    let rewrite = results.iter().find(|r| r.name == "rewrite_packed_index").unwrap();
    let scalar = results.iter().find(|r| r.name == "write_instance_scalar").unwrap();
    assert!(
        rewrite.metric("write_bytes").unwrap_or(0) > scalar.metric("write_bytes").unwrap_or(0),
        "rewriting a 4 KB index must write more than a scalar"
    );

    write_group_json("storage", &results);
    println!("primitive checks passed");
}
