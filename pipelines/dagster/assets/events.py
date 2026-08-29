"""
Dagster Software-Defined Assets for Contract Events (#523)
"""

from dagster import asset, AssetExecutionContext, Output, MetadataValue
from typing import List, Dict, Any
from ..resources.ledger_resource import LedgerRpcResource

@asset(
    group_name="raw_layer",
    description="Raw contract events extracted directly from Soroban RPC",
    compute_kind="python",
)
def raw_contract_events(context: AssetExecutionContext, ledger_rpc: LedgerRpcResource) -> Output[List[Dict[str, Any]]]:
    events = ledger_rpc.fetch_events_range(1, 100)
    context.log.info(f"Ingested {len(events)} raw events.")
    return Output(
        events,
        metadata={
            "record_count": MetadataValue.int(len(events)),
            "source_contract": MetadataValue.text(ledger_rpc.contract_id),
        },
    )

@asset(
    group_name="silver_layer",
    description="Cleaned, typed, and schema-validated audit events",
    compute_kind="pandas",
)
def silver_clean_events(context: AssetExecutionContext, raw_contract_events: List[Dict[str, Any]]) -> Output[List[Dict[str, Any]]]:
    cleaned = []
    for e in raw_contract_events:
        if e.get("index") is not None and e.get("event_hash"):
            cleaned.append({
                **e,
                "cleaned_timestamp": e["timestamp"],
                "is_valid": True,
            })
    return Output(
        cleaned,
        metadata={
            "valid_records": MetadataValue.int(len(cleaned)),
            "dropped_records": MetadataValue.int(len(raw_contract_events) - len(cleaned)),
        },
    )

@asset(
    group_name="gold_layer",
    description="Aggregated daily metrics and compliance summaries",
    compute_kind="duckdb",
)
def gold_daily_audit_summary(context: AssetExecutionContext, silver_clean_events: List[Dict[str, Any]]) -> Output[Dict[str, Any]]:
    summary = {
        "total_events": len(silver_clean_events),
        "unique_submitters": len(set(e["submitter"] for e in silver_clean_events)),
        "generation_time": int(context.instance.get_current_timestamp()),
    }
    return Output(summary, metadata={"summary": MetadataValue.json(summary)})
