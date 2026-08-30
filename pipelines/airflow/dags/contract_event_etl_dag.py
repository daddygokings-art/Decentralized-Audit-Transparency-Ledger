"""
Airflow Hourly ETL DAG for Contract Events (#523)
"""

from datetime import datetime, timedelta
from airflow import DAG
from airflow.operators.python import PythonOperator
from plugins.audit_ledger_operator import (
    AuditLedgerExtractOperator,
    DataQualityCheckOperator,
    AuditLedgerWarehouseLoaderOperator,
)

default_args = {
    "owner": "data-engineering",
    "depends_on_past": False,
    "email_on_failure": True,
    "email": ["alerts@audit-ledger.network"],
    "retries": 3,
    "retry_delay": timedelta(minutes=5),
}

with DAG(
    dag_id="contract_event_hourly_etl",
    default_args=default_args,
    description="Hourly ETL extracting Soroban contract events, executing data quality checks, and loading warehouse",
    schedule_interval="@hourly",
    start_date=datetime(2024, 1, 1),
    catchup=False,
    max_active_runs=1,
    tags=["audit-ledger", "etl", "hourly"],
) as dag:

    extract_events = AuditLedgerExtractOperator(
        task_id="extract_events",
        rpc_url="https://soroban-rpc.mainnet.stellar.org",
        batch_size=5000,
    )

    quality_check = DataQualityCheckOperator(
        task_id="data_quality_check",
        dataset_name="hourly_raw_events",
    )

    load_warehouse = AuditLedgerWarehouseLoaderOperator(
        task_id="load_warehouse",
        target_table="raw_contract_events",
        warehouse_type="snowflake",
    )

    extract_events >> quality_check >> load_warehouse
