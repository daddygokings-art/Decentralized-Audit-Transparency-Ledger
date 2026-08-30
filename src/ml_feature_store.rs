//! Contract Event ML Feature Store Module (#524)
//!
//! Provides on-chain coordination, feature view registration, cryptographic
//! feature attestations, and model drift alerts for ML pipelines.

use soroban_sdk::{
    contracttype, symbol_short, BytesN, Env, Symbol, Vec,
};

/// Data type of a feature value
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FeatureDataType {
    Float,
    Int,
    StringVal,
    BytesVal,
    VectorEmbedding,
}

/// Feature view metadata registration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeatureViewRecord {
    pub view_id: Symbol,
    pub entity_name: Symbol,
    pub feature_names: Vec<Symbol>,
    pub version: u32,
    pub ttl_seconds: u64,
    pub registered_at: u64,
    pub active: bool,
}

/// On-chain cryptographic attestation of computed feature values
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeatureAttestation {
    pub attestation_id: BytesN<32>,
    pub view_id: Symbol,
    pub entity_id: Symbol,
    pub feature_values_hash: BytesN<32>,
    pub attested_at: u64,
}

/// On-chain alert for detected feature drift or data distribution shift
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeatureDriftAlert {
    pub alert_id: BytesN<32>,
    pub view_id: Symbol,
    pub feature_name: Symbol,
    pub drift_score_ppm: u32, // Parts per million (e.g. 250000 = 0.25 PSI)
    pub threshold_ppm: u32,
    pub detected_at: u64,
}

const FEAT_VIEW_KEY: Symbol = symbol_short!("FT_VIEW");
const FEAT_ATT_KEY: Symbol = symbol_short!("FT_ATT");
const FEAT_DFT_KEY: Symbol = symbol_short!("FT_DFT");
const FEAT_LIST: Symbol = symbol_short!("FT_LIST");

/// Registers a new ML feature view definition
pub fn register_feature_view(
    env: &Env,
    view_id: Symbol,
    entity_name: Symbol,
    feature_names: Vec<Symbol>,
    version: u32,
    ttl_seconds: u64,
) -> FeatureViewRecord {
    let now = env.ledger().timestamp();
    let record = FeatureViewRecord {
        view_id: view_id.clone(),
        entity_name,
        feature_names,
        version,
        ttl_seconds,
        registered_at: now,
        active: true,
    };

    env.storage().persistent().set(&(FEAT_VIEW_KEY, view_id.clone()), &record);

    let mut list: Vec<Symbol> = env
        .storage()
        .persistent()
        .get(&FEAT_LIST)
        .unwrap_or_else(|| Vec::new(env));
    
    let mut exists = false;
    for i in 0..list.len() {
        if list.get(i).unwrap() == view_id {
            exists = true;
            break;
        }
    }
    if !exists {
        list.push_back(view_id);
        env.storage().persistent().set(&FEAT_LIST, &list);
    }

    record
}

/// Records a cryptographic attestation of feature values for an entity
pub fn record_feature_attestation(
    env: &Env,
    attestation_id: BytesN<32>,
    view_id: Symbol,
    entity_id: Symbol,
    feature_values_hash: BytesN<32>,
) -> FeatureAttestation {
    let now = env.ledger().timestamp();
    let attestation = FeatureAttestation {
        attestation_id: attestation_id.clone(),
        view_id,
        entity_id,
        feature_values_hash,
        attested_at: now,
    };

    env.storage().persistent().set(&(FEAT_ATT_KEY, attestation_id), &attestation);
    attestation
}

/// Records an on-chain alert for feature distribution drift
pub fn record_drift_alert(
    env: &Env,
    alert_id: BytesN<32>,
    view_id: Symbol,
    feature_name: Symbol,
    drift_score_ppm: u32,
    threshold_ppm: u32,
) -> FeatureDriftAlert {
    let now = env.ledger().timestamp();
    let alert = FeatureDriftAlert {
        alert_id: alert_id.clone(),
        view_id,
        feature_name,
        drift_score_ppm,
        threshold_ppm,
        detected_at: now,
    };

    env.storage().persistent().set(&(FEAT_DFT_KEY, alert_id), &alert);
    alert
}

/// Gets a feature view record by ID
pub fn get_feature_view(env: &Env, view_id: Symbol) -> Option<FeatureViewRecord> {
    env.storage().persistent().get(&(FEAT_VIEW_KEY, view_id))
}

/// Gets a feature attestation by ID
pub fn get_feature_attestation(env: &Env, attestation_id: BytesN<32>) -> Option<FeatureAttestation> {
    env.storage().persistent().get(&(FEAT_ATT_KEY, attestation_id))
}

/// Gets a feature drift alert by ID
pub fn get_drift_alert(env: &Env, alert_id: BytesN<32>) -> Option<FeatureDriftAlert> {
    env.storage().persistent().get(&(FEAT_DFT_KEY, alert_id))
}
