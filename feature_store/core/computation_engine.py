"""
Feature Computation Engine (#524)
"""

import time
import math
from typing import List, Dict, Any

class FeatureComputationEngine:
    @staticmethod
    def compute_submitter_features(events: List[Dict[str, Any]], now: Optional[float] = None) -> Dict[str, float]:
        if now is None:
            now = time.time()

        count_1h = 0
        count_24h = 0
        total_metadata_bytes = 0
        unique_categories = set()

        for e in events:
            event_ts = e.get("timestamp", 0)
            age_sec = now - event_ts

            if age_sec <= 3600:
                count_1h += 1
            if age_sec <= 86400:
                count_24h += 1
                meta = str(e.get("metadata", ""))
                total_metadata_bytes += len(meta)
                if "category" in e:
                    unique_categories.add(e["category"])

        avg_bytes = total_metadata_bytes / max(count_24h, 1)
        burst_ratio = count_1h / max(count_24h / 24.0, 0.1)

        return {
            "tx_count_1h": float(count_1h),
            "tx_count_24h": float(count_24h),
            "avg_metadata_bytes_24h": float(avg_bytes),
            "burst_ratio_1h_to_24h": float(burst_ratio),
            "unique_categories_count": float(len(unique_categories)),
            "velocity_anomaly_score": float(min(burst_ratio * 10.0, 100.0)),
        }
