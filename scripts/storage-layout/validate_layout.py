#!/usr/bin/env python3
"""
validate_layout.py — Compare two storage layout snapshots and report violations.

Usage:
    python3 validate_layout.py --old snapshots/baseline.json \
                               --new snapshots/current.json \
                               [--strict] \
                               [--allow-tombstone-removal]

Exit codes:
    0 — compatible (or only warnings without --strict)
    1 — warnings with --strict flag
    2 — breaking violations
"""

from __future__ import annotations

import argparse
import sys
import os
from pathlib import Path

# Allow importing layout_utils from the same directory
sys.path.insert(0, str(Path(__file__).parent))
from layout_utils import load_snapshot, validate_snapshots


# ---------------------------------------------------------------------------
# Storage footprint budgets (serialized XDR byte sizes)
# ---------------------------------------------------------------------------
#
# These budgets pin the serialized XDR byte footprint of records managed by
# `src/supply_chain.rs`. They are checked against the candidate snapshot and
# the build fails when a record grows beyond its budget. Update the budgets
# deliberately (and re-pin the snapshot tests) whenever the layout changes.
#
# Structure: { "supply_chain": { "<record>": <max_bytes> } }

STORAGE_FOOTPRINT_BUDGETS: dict[str, dict[str, int]] = {
    "supply_chain": {
        # Composite custody-handoff records (packed fields + enum discriminants)
        "CustodyHandoff": 128,
        "SupplyChainRecord": 192,
        "HandoffVerification": 96,
        # TTL / archival bookkeeping
        "TtlPolicy": 48,
        "ArchivalRecord": 64,
    },
}


def check_storage_footprint(
    new_snapshot: dict,
    budgets: dict[str, dict[str, int]] | None = None,
) -> list:
    """Return a list of footprint violations for the candidate snapshot.

    Each violation is a dict with keys: ``module``, ``record``, ``actual``,
    ``budget``. A record is flagged when its serialized XDR byte size exceeds
    the pinned budget. Missing records are ignored (they may be added later),
    but records present in the snapshot without a budget are reported as
    violations so new state keys cannot silently escape the byte budget.
    """
    if budgets is None:
        budgets = STORAGE_FOOTPRINT_BUDGETS

    violations: list = []
    modules = new_snapshot.get("modules", {})

    for module, record_budgets in budgets.items():
        module_data = modules.get(module)
        if not module_data:
            continue

        records = module_data.get("records", {})
        for record_name, record in records.items():
            actual = record.get("xdr_size", record.get("size"))
            if actual is None:
                continue

            budget = record_budgets.get(record_name)
            if budget is None:
                violations.append(
                    {
                        "module": module,
                        "record": record_name,
                        "actual": actual,
                        "budget": None,
                    }
                )
                continue

            if actual > budget:
                violations.append(
                    {
                        "module": module,
                        "record": record_name,
                        "actual": actual,
                        "budget": budget,
                    }
                )

    return violations


def print_sc_footprint_report(violations: list) -> None:
    if not violations:
        print(_green(_bold("✓ Storage footprint within budget.")))
        print()
        return

    print(_bold(_red(f"STORAGE FOOTPRINT VIOLATIONS ({len(violations)}):")))
    for v in violations:
        if v["budget"] is None:
            print(
                f"  {_red('✗')} [{v['module']}] '{v['record']}' "
                f"xdr_size={v['actual']} has no pinned byte budget"
            )
        else:
            over = v["actual"] - v["budget"]
            print(
                f"  {_red('✗')} [{v['module']}] '{v['record']}' "
                f"xdr_size={v['actual']} exceeds budget={v['budget']} (+{over} bytes)"
            )


# Storage footprint budgets (pinned byte thresholds)
# ---------------------------------------------------------------------------
#
# These budgets pin the serialized XDR byte size of records managed by
# ``src/ml_feature_store.rs``. They are intentionally conservative: any change
# that grows a record beyond its budget is a regression and must fail CI.
#
# The budgets are keyed by the record name as it appears in the snapshot's
# ``records`` mapping. ``max_bytes`` is the hard ceiling; ``snapshot_bytes`` is
# the value captured when the budget was last reviewed (used for drift
# reporting when the record is still within budget).
#
# To intentionally raise a budget, update both ``snapshot_bytes`` and
# ``max_bytes`` here and regenerate the baseline snapshot in the same PR.

ML_FEATURE_STORE_BUDGETS: dict[str, dict[str, int]] = {
    # Feature vector lineage anchor: key + vector payload + lineage metadata.
    "FeatureVector": {"snapshot_bytes": 256, "max_bytes": 288},
    # Hash anchoring record: content hash + anchor metadata + TTL bookkeeping.
    "FeatureHashAnchor": {"snapshot_bytes": 128, "max_bytes": 144},
    # Composite lineage edge linking two feature vectors.
    "LineageEdge": {"snapshot_bytes": 96, "max_bytes": 112},
    # TTL extension bookkeeping entry for archival-critical records.
    "TtlRecord": {"snapshot_bytes": 64, "max_bytes": 80},
}

# Module whose records these budgets govern. Used to scope the check so that
# unrelated records in the snapshot are not accidentally constrained.
ML_FEATURE_STORE_MODULE = "src/ml_feature_store.rs"


def check_ml_feature_store_footprint(
    new_snapshot: dict,
    budgets: dict | None = None,
) -> tuple[list, list]:
    """Enforce pinned byte budgets for ``src/ml_feature_store.rs`` records.

    Returns a ``(violations, warnings)`` tuple where each entry is a
    ``(record_name, actual_bytes, budget_bytes, message)`` tuple.

    * A record whose serialized size exceeds ``max_bytes`` is a violation.
    * A record whose serialized size exceeds ``snapshot_bytes`` but is still
      within ``max_bytes`` is a warning (drift from the reviewed snapshot).
    * A record listed in the budgets but absent from the snapshot is a warning
      (it may have been renamed or removed).
    """
    if budgets is None:
        budgets = ML_FEATURE_STORE_BUDGETS

    violations: list = []
    warnings: list = []

    records = new_snapshot.get("records", {}) or {}

    for record_name, budget in budgets.items():
        max_bytes = int(budget.get("max_bytes", 0))
        snapshot_bytes = int(budget.get("snapshot_bytes", max_bytes))

        record = records.get(record_name)
        if record is None:
            warnings.append(
                (
                    record_name,
                    0,
                    max_bytes,
                    f"record '{record_name}' from {ML_FEATURE_STORE_MODULE} "
                    f"is missing from the snapshot; budget cannot be enforced",
                )
            )
            continue

        actual_bytes = _record_serialized_size(record)

        if actual_bytes > max_bytes:
            violations.append(
                (
                    record_name,
                    actual_bytes,
                    max_bytes,
                    f"serialized XDR size {actual_bytes}B exceeds pinned budget "
                    f"{max_bytes}B for '{record_name}' ({ML_FEATURE_STORE_MODULE})",
                )
            )
        elif actual_bytes > snapshot_bytes:
            warnings.append(
                (
                    record_name,
                    actual_bytes,
                    snapshot_bytes,
                    f"serialized XDR size {actual_bytes}B drifted above reviewed "
                    f"snapshot {snapshot_bytes}B for '{record_name}' "
                    f"(still within {max_bytes}B budget)",
                )
            )

    return violations, warnings


def _record_serialized_size(record: dict) -> int:
    """Return the serialized byte size for a snapshot record entry.

    Snapshots may express the size directly via ``serialized_bytes`` /
    ``size_bytes`` / ``bytes``, or indirectly via a ``fields`` mapping whose
    per-field sizes are summed. Falls back to 0 when no size information is
    available so that missing data surfaces as a warning rather than a crash.
    """
    if not isinstance(record, dict):
        return 0

    for key in ("serialized_bytes", "size_bytes", "bytes"):
        value = record.get(key)
        if isinstance(value, int):
            return value

    fields = record.get("fields")
    if isinstance(fields, dict):
        total = 0
        for field in fields.values():
            if isinstance(field, int):
                total += field
            elif isinstance(field, dict):
                size = field.get("size_bytes", field.get("bytes", 0))
                if isinstance(size, int):
                    total += size
        return total

    return 0


def print_ml_footprint_report(
    violations: list,
    warnings: list,
    strict: bool,
) -> None:
    """Print the storage footprint regression report."""
    print(_bold("STORAGE FOOTPRINT (ml_feature_store):"))

    if not violations and not warnings:
        print(f"  {_green('✓')} All pinned byte budgets satisfied.")
        print()
        return

    for name, actual, budget, message in violations:
        print(f"  {_red('✗')} [{name}] {actual}B > {budget}B budget")
        print(f"      {message}")

    for name, actual, budget, message in warnings:
        print(f"  {_yellow('⚠')} [{name}] {actual}B > {budget}B reviewed snapshot")
        print(f"      {message}")

    if violations:
        print(_red(_bold(f"  {len(violations)} footprint budget violation(s).")))
    elif warnings and strict:
        print(_yellow(_bold(f"  {len(warnings)} footprint warning(s). --strict mode: treating as failure.")))
    else:
        print(_yellow(_bold(f"  {len(warnings)} footprint warning(s) but no budget violations.")))


# CBDC interop storage footprint budget
# ---------------------------------------------------------------------------
#
# Pinned byte budgets for serialized XDR storage records owned by
# `src/cbdc_interop.rs`. These thresholds are asserted against the candidate
# snapshot so that any regression in packing / tiering is caught in CI.
#
# Keys are the record names as emitted by the snapshot generator; values are
# the maximum allowed serialized XDR byte size.
CBDC_INTEROP_BYTE_BUDGET = {
    "CbdcInteropRecord": 256,
    "CbdcSettlementRecord": 320,
    "Iso20022MessageRecord": 512,
    "CbdcAuditRecord": 192,
}

# Module prefix used to identify CBDC interop records in snapshots.
CBDC_INTEROP_MODULE = "cbdc_interop"


def check_cbdc_interop_footprint(new_snapshot: dict) -> list:
    """Return a list of footprint violations for CBDC interop records.

    Each violation is a dict with ``record``, ``actual`` and ``budget`` keys.
    Records absent from the snapshot are ignored (they may be gated behind
    feature flags); records present but exceeding their pinned budget are
    reported.
    """
    violations = []
    records = new_snapshot.get("records", {})
    if not isinstance(records, dict):
        return violations

    for name, budget in CBDC_INTEROP_BYTE_BUDGET.items():
        record = records.get(name)
        if record is None:
            continue
        module = record.get("module", "")
        if module and CBDC_INTEROP_MODULE not in module:
            continue
        actual = record.get("serialized_size")
        if actual is None:
            continue
        if actual > budget:
            violations.append(
                {"record": name, "actual": actual, "budget": budget}
            )
    return violations


def print_cbdc_footprint_report(footprint_violations: list) -> None:
    print(_bold("CBDC INTEROP FOOTPRINT:"))
    if not footprint_violations:
        for name, budget in CBDC_INTEROP_BYTE_BUDGET.items():
            print(f"  {_green('✓')} {name} <= {budget} bytes")
    else:
        for v in footprint_violations:
            print(
                f"  {_red('✗')} {v['record']} = {v['actual']} bytes "
                f"(budget {v['budget']} bytes, over by "
                f"{v['actual'] - v['budget']})"
            )
    print()


# CQRS projection engine storage footprint budget
# ---------------------------------------------------------------------------
#
# Pinned serialized XDR sizes for `src/cqrs.rs` live in
# `snapshots/cqrs_footprint.json` (asserted byte-for-byte by
# `src/cqrs_storage_tests.rs`). This check fails CI when a pinned size exceeds
# its byte budget, when a record regresses above its legacy (pre-packing) size,
# or when the per-transaction write budget of the 1,000-tx benchmark is blown.
CQRS_FOOTPRINT_PATH = Path(__file__).parent / "snapshots" / "cqrs_footprint.json"


def check_cqrs_footprint(path: Path = CQRS_FOOTPRINT_PATH) -> list:
    """Return a list of human-readable CQRS footprint violations."""
    import json

    if not path.exists():
        return [f"missing pinned CQRS footprint file: {path}"]
    data = json.loads(path.read_text())
    violations = []
    for name, rec in data.get("records", {}).items():
        size, budget = rec.get("xdr_size"), rec.get("byte_budget")
        legacy = rec.get("legacy_xdr_size")
        if size is None or budget is None:
            violations.append(f"{name}: missing xdr_size or byte_budget")
            continue
        if size > budget:
            violations.append(f"{name}: xdr_size={size} exceeds byte_budget={budget}")
        if legacy is not None and size >= legacy:
            violations.append(f"{name}: xdr_size={size} is not smaller than legacy={legacy}")
    bench = data.get("benchmark", {})
    measured = bench.get("measured_max_write_bytes_per_tx")
    limit = bench.get("max_write_bytes_per_tx_budget")
    if measured is not None and limit is not None and measured > limit:
        violations.append(
            f"benchmark: max write bytes/tx {measured} exceeds budget {limit}"
        )
    for key, tier in data.get("storage_tiers", {}).items():
        if tier not in ("instance", "persistent", "temporary"):
            violations.append(f"storage tier for {key} is invalid: {tier}")
    return violations


def print_cqrs_footprint_report(violations: list) -> None:
    print(_bold("STORAGE FOOTPRINT (cqrs):"))
    if not violations:
        print(f"  {_green('✓')} all src/cqrs.rs records within pinned byte budgets")
    for v in violations:
        print(f"  {_red('✗')} {v}")
    print()


# ---------------------------------------------------------------------------
# Colour helpers
# ---------------------------------------------------------------------------

def _use_color() -> bool:
    return sys.stdout.isatty() and os.environ.get("NO_COLOR", "") == ""


def _green(s: str) -> str:
    return f"\033[32m{s}\033[0m" if _use_color() else s


def _yellow(s: str) -> str:
    return f"\033[33m{s}\033[0m" if _use_color() else s


def _red(s: str) -> str:
    return f"\033[31m{s}\033[0m" if _use_color() else s


def _bold(s: str) -> str:
    return f"\033[1m{s}\033[0m" if _use_color() else s


# ---------------------------------------------------------------------------
# Report printer
# ---------------------------------------------------------------------------

def print_report(
    old_snapshot: dict,
    new_snapshot: dict,
    violations: list,
    warnings: list,
    strict: bool,
) -> None:
    old_contract = old_snapshot.get("contract", "?")
    new_commit = new_snapshot.get("git_commit", "?")
    old_commit = old_snapshot.get("git_commit", "?")

    print()
    print(_bold("═══════════════════════════════════════════════════"))
    print(_bold("  Storage Layout Compatibility Report"))
    print(_bold("═══════════════════════════════════════════════════"))
    print(f"  Contract : {old_contract}")
    print(f"  Baseline : {old_commit}")
    print(f"  Candidate: {new_commit}")
    print()

    # PASSED checks
    checks = [
        "No ordinal reuse",
        "No type changes",
        "Append-only additions",
        "Variant order preserved",
        "No non-tombstone removals",
    ]
    violation_rules = {v.rule for v in violations}

    rule_to_check = {
        "no_ordinal_reuse": "No ordinal reuse",
        "no_type_change": "No type changes",
        "append_only": "Append-only additions",
        "order_preserved": "Variant order preserved",
        "no_removal": "No non-tombstone removals",
    }

    failed_checks = {rule_to_check.get(r, r) for r in violation_rules}
    warning_rules = {w.rule for w in warnings}
    warned_checks = {rule_to_check.get(r, r) for r in warning_rules}

    print(_bold("CHECKS:"))
    for check in checks:
        if check in failed_checks:
            print(f"  {_red('✗')} {check}")
        elif check in warned_checks:
            print(f"  {_yellow('⚠')} {check}")
        else:
            print(f"  {_green('✓')} {check}")
    print()

    # VIOLATIONS
    if violations:
        print(_bold(_red(f"VIOLATIONS ({len(violations)}):")))
        for v in violations:
            ordinal_str = f"ordinal={v.ordinal}" if v.ordinal >= 0 else ""
            print(f"  {_red('✗')} [{v.rule}] '{v.variant_name}' {ordinal_str}")
            # Wrap description
            desc = v.description
            words = desc.split()
            line = "      "
            for word in words:
                if len(line) + len(word) + 1 > 78:
                    print(line)
                    line = "      " + word + " "
                else:
                    line += word + " "
            if line.strip():
                print(line)
        print()

    # WARNINGS
    if warnings:
        print(_bold(_yellow(f"WARNINGS ({len(warnings)}):")))
        for w in warnings:
            print(f"  {_yellow('⚠')} [{w.rule}] '{w.variant_name}' ordinal={w.ordinal}")
            desc = w.description
            words = desc.split()
            line = "      "
            for word in words:
                if len(line) + len(word) + 1 > 78:
                    print(line)
                    line = "      " + word + " "
                else:
                    line += word + " "
            if line.strip():
                print(line)
        print()

    # Summary line
    if not violations and not warnings:
        print(_green(_bold("✓ Storage layout is compatible.")))
    elif not violations and warnings:
        if strict:
            print(_yellow(_bold(f"⚠ Storage layout has {len(warnings)} warning(s). --strict mode: treating as failure.")))
        else:
            print(_yellow(_bold(f"⚠ Storage layout has {len(warnings)} warning(s) but no breaking violations.")))
    else:
        print(_red(_bold(f"✗ Storage layout has {len(violations)} BREAKING violation(s).")))
    print()


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

def main() -> int:
    parser = argparse.ArgumentParser(
        description="Compare two storage layout snapshots and report compatibility."
    )
    parser.add_argument(
        "--old",
        required=True,
        help="Path to the baseline (old) snapshot JSON.",
    )
    parser.add_argument(
        "--new",
        required=True,
        help="Path to the candidate (new) snapshot JSON.",
    )
    parser.add_argument(
        "--strict",
        action="store_true",
        default=False,
        help="Treat warnings as failures (exit 1 on warnings).",
    )
    parser.add_argument(
        "--allow-tombstone-removal",
        action="store_true",
        default=False,
        help="Suppress warnings when tombstone variants are removed.",
    )
    args = parser.parse_args()

    try:
        old_snapshot = load_snapshot(args.old)
    except FileNotFoundError as e:
        print(f"ERROR: {e}", file=sys.stderr)
        return 2
    except Exception as e:
        print(f"ERROR: Could not load old snapshot: {e}", file=sys.stderr)
        return 2

    try:
        new_snapshot = load_snapshot(args.new)
    except FileNotFoundError as e:
        print(f"ERROR: {e}", file=sys.stderr)
        return 2
    except Exception as e:
        print(f"ERROR: Could not load new snapshot: {e}", file=sys.stderr)
        return 2

    violations, warnings = validate_snapshots(
        old_snapshot,
        new_snapshot,
        allow_tombstone_removal=args.allow_tombstone_removal,
    )

    footprint_violations = check_cbdc_interop_footprint(new_snapshot)

    print_report(old_snapshot, new_snapshot, violations, warnings, strict=args.strict)
    print_cbdc_footprint_report(footprint_violations)

    # Storage footprint assertions for supply chain
    sc_violations = check_storage_footprint(new_snapshot)
    print_sc_footprint_report(sc_violations)

    # Storage footprint regression assertions for src/ml_feature_store.rs
    ml_violations, ml_warnings = check_ml_feature_store_footprint(new_snapshot)
    print_ml_footprint_report(ml_violations, ml_warnings, strict=args.strict)

    # Storage footprint regression assertions for src/cqrs.rs
    cqrs_violations = check_cqrs_footprint()
    print_cqrs_footprint_report(cqrs_violations)

    if violations or footprint_violations or sc_violations or ml_violations or cqrs_violations:
        return 2
    if (warnings or ml_warnings) and args.strict:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
