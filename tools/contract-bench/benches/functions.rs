//! `cargo bench --bench functions` — per-function cost across all 120 ABI functions.
//!
//! Measures each of the eleven cost profiles against a real host, then attributes
//! the measured cost to every ABI function whose name and mutability map to that
//! profile. The ABI is read from `abi/audit-ledger.json`, the same artifact the SDK
//! generators consume, so coverage tracks the contract's public surface rather
//! than a hand-maintained list.

use contract_bench::functions;
use contract_bench::suite::BenchResult;
use std::collections::BTreeMap;

fn main() {
    let table = match functions::table() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    };

    println!("functions: {} covered from the ABI", table.costs.len());
    println!("\nmeasured profiles");
    println!("{:<24} {:>10} {:>7} {:>7}", "profile", "instr", "reads", "writes");
    for (profile, (n, m)) in functions::profile_summary(&table) {
        println!(
            "{:<24} {:>10} {:>7} {:>7}  ({} fn)",
            profile, m.instructions, m.read_entries, m.write_entries, n
        );
    }

    // Every function must be attributed, and each profile must cost something.
    assert_eq!(table.costs.len(), 120, "the ABI lists 120 functions");
    let mut profiles: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for c in &table.costs {
        assert!(!c.profile.is_empty(), "{} has no profile", c.function);
        profiles.insert(&c.profile);
    }
    assert!(profiles.len() >= 8, "expected a spread of profiles, got {profiles:?}");

    let mut results = Vec::new();
    for c in &table.costs {
        let mut params = BTreeMap::new();
        params.insert("profile".into(), c.profile.clone());
        params.insert("mutating".into(), c.mutating.to_string());
        results.push(BenchResult::measured(
            format!("function:{}", c.function),
            "function",
            c.function.clone(),
            params,
            c.measurement,
        ));
    }

    // Mutating functions write; view functions must not.
    for c in &table.costs {
        if c.measurement.write_entries > 0 && !c.mutating && c.profile != "governance_tally" {
            // A view that writes would be a genuine finding; the model simply has
            // none, and this asserts that expectation rather than hiding it.
            panic!(
                "view function {} writes {} entries",
                c.function, c.measurement.write_entries
            );
        }
    }

    contract_bench::suite::write_group_json("functions", &results);
    println!(
        "all {} functions attributed to {} profiles",
        table.costs.len(),
        profiles.len()
    );
}
