//! `cargo bench --bench scenarios` — the five required contract scenarios.
//!
//! A custom harness rather than libtest, for the reason set out in
//! `src/lib.rs`: the metrics are host-metered and deterministic, so there is no
//! wall-clock distribution to estimate and no reason to pay the variance. Each
//! scenario is measured on a freshly seeded ledger and the host's own counters
//! are read back.
//!
//! The assertions at the end are the ones worth keeping: they check the *shape* of
//! the cost model rather than pinning absolute numbers, so they stay meaningful
//! when the SDK's price list changes but still fail if a scenario stops doing the
//! work it claims to do.

use contract_bench::scenarios;
use contract_bench::suite::BenchResult;

fn main() {
    let cases = scenarios::all();
    println!("scenarios: {} cases", cases.len());
    println!(
        "{:<52} {:>10} {:>7} {:>7} {:>10} {:>10}",
        "case", "instr", "reads", "writes", "write_B", "rent"
    );

    let mut results = Vec::new();
    for case in &cases {
        // Measured twice and compared, so a run whose own numbers moved cannot
        // quietly become the next baseline.
        let m = case.assert_repeatable();
        println!(
            "{:<52} {:>10} {:>7} {:>7} {:>10} {:>10}",
            case.id, m.instructions, m.read_entries, m.write_entries, m.write_bytes, m.rent_bumps
        );
        results.push(BenchResult::measured(
            &case.id,
            &case.group,
            &case.name,
            case.params(),
            m,
        ));
    }

    shape_checks(&results);
    contract_bench::suite::write_group_json("scenarios", &results);
}

/// Assertions on the shape of the numbers rather than their magnitude.
fn shape_checks(results: &[BenchResult]) {
    let find = |name: &str| {
        results
            .iter()
            .find(|r| r.name == name)
            .unwrap_or_else(|| panic!("missing scenario {name}"))
    };

    for r in results {
        assert!(
            r.metric("instructions").unwrap_or(0) > 0,
            "{} metered no instructions",
            r.id
        );
    }

    let query = find("queries");
    assert_eq!(
        query.metric("write_entries"),
        Some(0),
        "a query must never write; if it does, the model has diverged from the contract"
    );
    assert!(query.metric("read_entries").unwrap_or(0) > 0, "a query must read");

    for name in ["single_event", "batch_events", "governance", "archive"] {
        assert!(find(name).metric("write_entries").unwrap_or(0) > 0, "{name} must write");
    }

    // Batch cost must grow with batch size *at a fixed ledger size*. Comparing
    // across ledger sizes would be comparing two variables at once: a sixteen
    // event batch on a small ledger costs more than a four event batch on a
    // large one, and there is nothing wrong with that.
    for ledger in scenarios::WRITE_LEDGER_SIZES {
        let mut batches: Vec<&BenchResult> = results
            .iter()
            .filter(|r| r.name == "batch_events" && r.id.contains(&format!("ledger={ledger}/")))
            .collect();
        batches.sort_by_key(|b| {
            b.id.rsplit("batch=")
                .next()
                .and_then(|n| n.parse::<u32>().ok())
                .unwrap_or(0)
        });
        assert!(
            batches.len() >= 2,
            "expected a batch ladder at ledger={ledger}, found {}",
            batches.len()
        );
        for pair in batches.windows(2) {
            assert!(
                pair[1].metric("instructions").unwrap_or(0) > pair[0].metric("instructions").unwrap_or(0),
                "batch cost must grow within ledger={ledger}: {} then {}",
                pair[0].id,
                pair[1].id
            );
        }
    }
    println!("shape checks passed");
}
