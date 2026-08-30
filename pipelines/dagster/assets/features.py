"""
Dagster Assets for Feature Engineering (#523)
"""

from dagster import asset, AssetExecutionContext, Output, MetadataValue
from typing import List, Dict, Any

@asset(
    group_name="ml_layer",
    description="Computed behavioral features for submitters across sliding windows",
    compute_kind="scikit-learn",
)
def submitter_behavioral_features(context: AssetExecutionContext, silver_clean_events: List[Dict[str, Any]]) -> Output[Dict[str, Any]]:
    features = {
        "feature_count": 12,
        "entity_count": len(silver_clean_events),
        "feature_names": ["tx_frequency_1h", "avg_metadata_bytes", "velocity_anomaly_score"],
    }
    return Output(features, metadata={"features_info": MetadataValue.json(features)})
