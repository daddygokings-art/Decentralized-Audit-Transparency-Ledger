//! Contract Event Serverless Functions for Event Processing (#522)
//!
//! Provides on-chain function registration, processing receipts,
//! routing rules management, and verification for serverless event
//! processors (AWS Lambda, Google Cloud Functions, Azure Functions, Knative).

use soroban_sdk::{
    contracttype, symbol_short, Bytes, BytesN, Env, Symbol, Vec,
};

/// Serverless platform provider
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServerlessProvider {
    AwsLambda,
    GoogleCloudFunctions,
    AzureFunctions,
    Knative,
    Custom,
}

/// Function processing role
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FunctionType {
    Transformer,
    Enricher,
    Filter,
    Router,
    CompositePipeline,
}

/// Serverless function registration record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerlessFunctionRecord {
    pub function_id: Symbol,
    pub provider: ServerlessProvider,
    pub function_type: FunctionType,
    pub version: u32,
    pub endpoint: Bytes,
    pub registered_at: u64,
    pub invocations_count: u64,
    pub active: bool,
}

/// On-chain receipt of serverless event processing
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessingReceipt {
    pub receipt_id: BytesN<32>,
    pub event_id: BytesN<32>,
    pub function_id: Symbol,
    pub input_hash: BytesN<32>,
    pub output_hash: BytesN<32>,
    pub execution_time_ms: u32,
    pub processed_at: u64,
    pub success: bool,
}

/// Declarative event routing rule
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoutingRule {
    pub rule_id: Symbol,
    pub event_pattern: Symbol,
    pub target_destination: Bytes,
    pub priority: u32,
    pub active: bool,
}

const FUNC_KEY: Symbol = symbol_short!("SRV_FUNC");
const RECEIPT_KEY: Symbol = symbol_short!("SRV_RCPT");
const RULE_KEY: Symbol = symbol_short!("SRV_RULE");
const FUNC_LIST: Symbol = symbol_short!("SRV_FLST");

/// Registers a serverless processing function on-chain
pub fn register_function(
    env: &Env,
    function_id: Symbol,
    provider: ServerlessProvider,
    function_type: FunctionType,
    version: u32,
    endpoint: Bytes,
) -> ServerlessFunctionRecord {
    let now = env.ledger().timestamp();
    let record = ServerlessFunctionRecord {
        function_id: function_id.clone(),
        provider,
        function_type,
        version,
        endpoint,
        registered_at: now,
        invocations_count: 0,
        active: true,
    };

    env.storage().persistent().set(&(FUNC_KEY, function_id.clone()), &record);

    let mut list: Vec<Symbol> = env
        .storage()
        .persistent()
        .get(&FUNC_LIST)
        .unwrap_or_else(|| Vec::new(env));
    
    let mut exists = false;
    for i in 0..list.len() {
        if list.get(i).unwrap() == function_id {
            exists = true;
            break;
        }
    }
    if !exists {
        list.push_back(function_id);
        env.storage().persistent().set(&FUNC_LIST, &list);
    }

    record
}

/// Records a processing receipt for auditing serverless event handling
pub fn record_processing_receipt(
    env: &Env,
    receipt_id: BytesN<32>,
    event_id: BytesN<32>,
    function_id: Symbol,
    input_hash: BytesN<32>,
    output_hash: BytesN<32>,
    execution_time_ms: u32,
    success: bool,
) -> ProcessingReceipt {
    let now = env.ledger().timestamp();
    let receipt = ProcessingReceipt {
        receipt_id: receipt_id.clone(),
        event_id,
        function_id: function_id.clone(),
        input_hash,
        output_hash,
        execution_time_ms,
        processed_at: now,
        success,
    };

    env.storage().persistent().set(&(RECEIPT_KEY, receipt_id), &receipt);

    // Increment invocation count on function
    let func_key = (FUNC_KEY, function_id);
    if let Some(mut func_record) = env.storage().persistent().get::<_, ServerlessFunctionRecord>(&func_key) {
        func_record.invocations_count = func_record.invocations_count.saturating_add(1);
        env.storage().persistent().set(&func_key, &func_record);
    }

    receipt
}

/// Sets an event routing rule
pub fn set_routing_rule(
    env: &Env,
    rule_id: Symbol,
    event_pattern: Symbol,
    target_destination: Bytes,
    priority: u32,
    active: bool,
) -> RoutingRule {
    let rule = RoutingRule {
        rule_id: rule_id.clone(),
        event_pattern,
        target_destination,
        priority,
        active,
    };

    env.storage().persistent().set(&(RULE_KEY, rule_id), &rule);
    rule
}

/// Gets a serverless function record by ID
pub fn get_function(env: &Env, function_id: Symbol) -> Option<ServerlessFunctionRecord> {
    env.storage().persistent().get(&(FUNC_KEY, function_id))
}

/// Gets a processing receipt by ID
pub fn get_processing_receipt(env: &Env, receipt_id: BytesN<32>) -> Option<ProcessingReceipt> {
    env.storage().persistent().get(&(RECEIPT_KEY, receipt_id))
}

/// Gets a routing rule by ID
pub fn get_routing_rule(env: &Env, rule_id: Symbol) -> Option<RoutingRule> {
    env.storage().persistent().get(&(RULE_KEY, rule_id))
}
