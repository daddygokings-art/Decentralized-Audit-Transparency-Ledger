//! Contract Event Data Pipeline Attestation Module (#523)
//!
//! Provides on-chain tracking for Airflow and Dagster data pipeline executions,
//! data quality check attestations, and warehouse synchronization checkpoints.

use soroban_sdk::{
    contracttype, symbol_short, BytesN, Env, Symbol, Vec,
};

/// Orchestration pipeline engine
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PipelineEngine {
    Airflow,
    Dagster,
    Custom,
}

/// Pipeline execution stage
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PipelineStage {
    Ingestion,
    Aggregation,
    FeatureEngineering,
    WarehouseLoad,
    DataQualityCheck,
}

/// Record of an executed data pipeline run
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PipelineRunRecord {
    pub run_id: Symbol,
    pub pipeline_name: Symbol,
    pub engine: PipelineEngine,
    pub events_processed: u64,
    pub start_time: u64,
    pub completed_time: u64,
    pub success: bool,
}

/// Attestation of data quality assertions (null checks, range checks, schema validation)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataQualityAttestation {
    pub attestation_id: BytesN<32>,
    pub dataset_name: Symbol,
    pub checks_passed: u32,
    pub checks_failed: u32,
    pub metrics_hash: BytesN<32>,
    pub attested_at: u64,
}

/// Data warehouse synchronization watermark checkpoint
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WarehouseCheckpoint {
    pub warehouse_name: Symbol,
    pub table_name: Symbol,
    pub last_synced_ledger: u64,
    pub last_synced_index: u64,
    pub last_sync_timestamp: u64,
}

const PIPE_RUN_KEY: Symbol = symbol_short!("PIP_RUN");
const DQ_ATT_KEY: Symbol = symbol_short!("DQ_ATT");
const WH_CHK_KEY: Symbol = symbol_short!("WH_CHK");
const PIPE_LIST: Symbol = symbol_short!("PIP_LIST");

/// Records a completed pipeline execution on-chain
pub fn record_pipeline_run(
    env: &Env,
    run_id: Symbol,
    pipeline_name: Symbol,
    engine: PipelineEngine,
    events_processed: u64,
    duration_seconds: u64,
    success: bool,
) -> PipelineRunRecord {
    let now = env.ledger().timestamp();
    let start_time = now.saturating_sub(duration_seconds);
    let record = PipelineRunRecord {
        run_id: run_id.clone(),
        pipeline_name,
        engine,
        events_processed,
        start_time,
        completed_time: now,
        success,
    };

    env.storage().persistent().set(&(PIPE_RUN_KEY, run_id.clone()), &record);

    let mut list: Vec<Symbol> = env
        .storage()
        .persistent()
        .get(&PIPE_LIST)
        .unwrap_or_else(|| Vec::new(env));
    
    let mut exists = false;
    for i in 0..list.len() {
        if list.get(i).unwrap() == run_id {
            exists = true;
            break;
        }
    }
    if !exists {
        list.push_back(run_id);
        env.storage().persistent().set(&PIPE_LIST, &list);
    }

    record
}

/// Records a cryptographic attestation of data quality assertions
pub fn attest_data_quality(
    env: &Env,
    attestation_id: BytesN<32>,
    dataset_name: Symbol,
    checks_passed: u32,
    checks_failed: u32,
    metrics_hash: BytesN<32>,
) -> DataQualityAttestation {
    let now = env.ledger().timestamp();
    let attestation = DataQualityAttestation {
        attestation_id: attestation_id.clone(),
        dataset_name,
        checks_passed,
        checks_failed,
        metrics_hash,
        attested_at: now,
    };

    env.storage()
        .persistent()
        .set(&(DQ_ATT_KEY, attestation_id), &attestation);

    attestation
}

/// Updates the sync watermark checkpoint for a warehouse table
pub fn update_warehouse_checkpoint(
    env: &Env,
    warehouse_name: Symbol,
    table_name: Symbol,
    last_synced_ledger: u64,
    last_synced_index: u64,
) -> WarehouseCheckpoint {
    let now = env.ledger().timestamp();
    let checkpoint = WarehouseCheckpoint {
        warehouse_name: warehouse_name.clone(),
        table_name: table_name.clone(),
        last_synced_ledger,
        last_synced_index,
        last_sync_timestamp: now,
    };

    let key = (WH_CHK_KEY, warehouse_name, table_name);
    env.storage().persistent().set(&key, &checkpoint);

    checkpoint
}

/// Gets a pipeline run record by ID
pub fn get_pipeline_run(env: &Env, run_id: Symbol) -> Option<PipelineRunRecord> {
    env.storage().persistent().get(&(PIPE_RUN_KEY, run_id))
}

/// Gets a data quality attestation by ID
pub fn get_data_quality_attestation(
    env: &Env,
    attestation_id: BytesN<32>,
) -> Option<DataQualityAttestation> {
    env.storage().persistent().get(&(DQ_ATT_KEY, attestation_id))
}

/// Gets a warehouse sync checkpoint
pub fn get_warehouse_checkpoint(
    env: &Env,
    warehouse_name: Symbol,
    table_name: Symbol,
) -> Option<WarehouseCheckpoint> {
    let key = (WH_CHK_KEY, warehouse_name, table_name);
    env.storage().persistent().get(&key)
}
