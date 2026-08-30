"""
Point-in-Time (AS-OF) Training Dataset Generator (#524)
"""

from typing import List, Dict, Any

class TrainingDataGenerator:
    """
    Prevents data leakage by ensuring features are joined AS-OF observation timestamp.
    """
    @staticmethod
    def point_in_time_join(
        observations: List[Dict[str, Any]],
        feature_history: List[Dict[str, Any]],
        entity_key: str = "submitter_address",
    ) -> List[Dict[str, Any]]:
        dataset = []

        for obs in observations:
            entity_val = obs[entity_key]
            obs_time = obs["timestamp"]

            # Filter features prior to or at observation timestamp
            matching_feats = [
                f for f in feature_history
                if f[entity_key] == entity_val and f["feature_timestamp"] <= obs_time
            ]
            matching_feats.sort(key=lambda x: x["feature_timestamp"], reverse=True)

            latest_feat = matching_feats[0] if matching_feats else {}
            merged = {**obs, **{k: v for k, v in latest_feat.items() if k not in obs}}
            dataset.append(merged)

        return dataset
