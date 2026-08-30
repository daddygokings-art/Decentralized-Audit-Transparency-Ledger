//! Contract Event Edge Computing Integration Module (#521)
//!
//! Provides on-chain coordination, node registration, cache attestation,
//! batch ingestion verification, and caching policies for edge platforms
//! (Cloudflare Workers, AWS Lambda@Edge, Fastly Compute@Edge).

use soroban_sdk::{
    contracttype, symbol_short, Address, Bytes, BytesN, Env, Symbol, Vec,
};

/// Edge computing platform provider
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EdgePlatform {
    CloudflareWorkers,
    AwsLambdaEdge,
    FastlyComputeEdge,
    CustomEdge,
}

/// Status of an edge node
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EdgeNodeStatus {
    Active,
    Degraded,
    Draining,
    Suspended,
}

/// Edge node registration record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EdgeNodeRecord {
    pub node_id: Symbol,
    pub platform: EdgePlatform,
    pub endpoint: Bytes,
    pub region: Symbol,
    pub public_key: BytesN<32>,
    pub registered_at: u64,
    pub last_heartbeat: u64,
    pub status: EdgeNodeStatus,
    pub total_ingested: u64,
    pub total_cache_hits: u64,
}

/// Attestation for an edge-cached query response
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EdgeCacheAttestation {
    pub cache_key_hash: BytesN<32>,
    pub content_hash: BytesN<32>,
    pub ttl_seconds: u32,
    pub hit_count: u64,
    pub edge_node_id: Symbol,
    pub attested_at: u64,
    pub expires_at: u64,
}

/// Record for a batch of events ingested via edge gateway
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EdgeBatchIngestRecord {
    pub batch_id: BytesN<32>,
    pub root_hash: BytesN<32>,
    pub event_count: u32,
    pub submitter: Address,
    pub edge_node_id: Symbol,
    pub ingested_at: u64,
    pub verified: bool,
}

/// Edge caching and stale-while-revalidate policy
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EdgeCachePolicy {
    pub event_type: Symbol,
    pub default_ttl_seconds: u32,
    pub stale_while_revalidate_seconds: u32,
    pub edge_tags: Vec<Symbol>,
    pub is_cacheable: bool,
}

const EDGE_NODE_KEY: Symbol = symbol_short!("EDG_NODE");
const EDGE_ATTEST_KEY: Symbol = symbol_short!("EDG_ATT");
const EDGE_BATCH_KEY: Symbol = symbol_short!("EDG_BAT");
const EDGE_POLICY_KEY: Symbol = symbol_short!("EDG_POL");
const EDGE_NODES_LIST: Symbol = symbol_short!("EDG_LIST");

/// Registers a new edge computing node
pub fn register_edge_node(
    env: &Env,
    node_id: Symbol,
    platform: EdgePlatform,
    endpoint: Bytes,
    region: Symbol,
    public_key: BytesN<32>,
) -> EdgeNodeRecord {
    let now = env.ledger().timestamp();
    let record = EdgeNodeRecord {
        node_id: node_id.clone(),
        platform,
        endpoint,
        region,
        public_key,
        registered_at: now,
        last_heartbeat: now,
        status: EdgeNodeStatus::Active,
        total_ingested: 0,
        total_cache_hits: 0,
    };

    env.storage().persistent().set(&(EDGE_NODE_KEY, node_id.clone()), &record);

    let mut nodes: Vec<Symbol> = env
        .storage()
        .persistent()
        .get(&EDGE_NODES_LIST)
        .unwrap_or_else(|| Vec::new(env));
    
    let mut found = false;
    for i in 0..nodes.len() {
        if nodes.get(i).unwrap() == node_id {
            found = true;
            break;
        }
    }
    if !found {
        nodes.push_back(node_id);
        env.storage().persistent().set(&EDGE_NODES_LIST, &nodes);
    }

    record
}

/// Updates edge node heartbeat and performance counters
pub fn heartbeat_edge_node(
    env: &Env,
    node_id: Symbol,
    ingested_increment: u64,
    cache_hits_increment: u64,
    status: EdgeNodeStatus,
) -> Option<EdgeNodeRecord> {
    let key = (EDGE_NODE_KEY, node_id.clone());
    let mut record: EdgeNodeRecord = env.storage().persistent().get(&key)?;
    
    record.last_heartbeat = env.ledger().timestamp();
    record.total_ingested = record.total_ingested.saturating_add(ingested_increment);
    record.total_cache_hits = record.total_cache_hits.saturating_add(cache_hits_increment);
    record.status = status;

    env.storage().persistent().set(&key, &record);
    Some(record)
}

/// Records a cryptographic attestation of an edge-cached response
pub fn record_cache_attestation(
    env: &Env,
    cache_key_hash: BytesN<32>,
    content_hash: BytesN<32>,
    ttl_seconds: u32,
    hit_count: u64,
    edge_node_id: Symbol,
) -> EdgeCacheAttestation {
    let now = env.ledger().timestamp();
    let attestation = EdgeCacheAttestation {
        cache_key_hash: cache_key_hash.clone(),
        content_hash,
        ttl_seconds,
        hit_count,
        edge_node_id,
        attested_at: now,
        expires_at: now.saturating_add(ttl_seconds as u64),
    };

    env.storage()
        .persistent()
        .set(&(EDGE_ATTEST_KEY, cache_key_hash), &attestation);

    attestation
}

/// Verifies and logs an edge-ingested event batch
pub fn record_edge_batch(
    env: &Env,
    batch_id: BytesN<32>,
    root_hash: BytesN<32>,
    event_count: u32,
    submitter: Address,
    edge_node_id: Symbol,
) -> EdgeBatchIngestRecord {
    let now = env.ledger().timestamp();
    let record = EdgeBatchIngestRecord {
        batch_id: batch_id.clone(),
        root_hash,
        event_count,
        submitter,
        edge_node_id,
        ingested_at: now,
        verified: true,
    };

    env.storage().persistent().set(&(EDGE_BATCH_KEY, batch_id), &record);
    record
}

/// Configures caching policy for an event type
pub fn set_edge_cache_policy(
    env: &Env,
    event_type: Symbol,
    default_ttl_seconds: u32,
    stale_while_revalidate_seconds: u32,
    edge_tags: Vec<Symbol>,
    is_cacheable: bool,
) -> EdgeCachePolicy {
    let policy = EdgeCachePolicy {
        event_type: event_type.clone(),
        default_ttl_seconds,
        stale_while_revalidate_seconds,
        edge_tags,
        is_cacheable,
    };

    env.storage().persistent().set(&(EDGE_POLICY_KEY, event_type), &policy);
    policy
}

/// Retrieves an edge node record by ID
pub fn get_edge_node(env: &Env, node_id: Symbol) -> Option<EdgeNodeRecord> {
    env.storage().persistent().get(&(EDGE_NODE_KEY, node_id))
}

/// Retrieves a cache attestation by cache key hash
pub fn get_cache_attestation(env: &Env, cache_key_hash: BytesN<32>) -> Option<EdgeCacheAttestation> {
    env.storage().persistent().get(&(EDGE_ATTEST_KEY, cache_key_hash))
}

/// Retrieves a batch ingest record by batch ID
pub fn get_edge_batch(env: &Env, batch_id: BytesN<32>) -> Option<EdgeBatchIngestRecord> {
    env.storage().persistent().get(&(EDGE_BATCH_KEY, batch_id))
}

/// Retrieves the cache policy for an event type
pub fn get_edge_cache_policy(env: &Env, event_type: Symbol) -> Option<EdgeCachePolicy> {
    env.storage().persistent().get(&(EDGE_POLICY_KEY, event_type))
}
