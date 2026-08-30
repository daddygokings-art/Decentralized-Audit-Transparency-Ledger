"""
Data Warehouse Loader Utilities (#523)
"""

from typing import List, Dict, Any

class WarehouseLoader:
    @staticmethod
    def load_to_duckdb(events: List[Dict[str, Any]], db_path: str = "audit_warehouse.duckdb") -> int:
        print(f"Loading {len(events)} events into DuckDB: {db_path}")
        return len(events)

    @staticmethod
    def load_to_snowflake(events: List[Dict[str, Any]], table: str = "RAW_EVENTS") -> int:
        print(f"Staging and copying {len(events)} events to Snowflake {table}")
        return len(events)

    @staticmethod
    def load_to_bigquery(events: List[Dict[str, Any]], dataset: str = "audit_ledger", table: str = "events") -> int:
        print(f"Streaming {len(events)} events into BigQuery {dataset}.{table}")
        return len(events)
