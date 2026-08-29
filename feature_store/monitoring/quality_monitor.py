"""
Feature Quality & Freshness Monitor (#524)
"""

import time
from typing import Dict, Any, List

class FeatureQualityMonitor:
    @staticmethod
    def assess_feature_quality(
        feature_name: str,
        values: List[Any],
        max_missing_pct: float = 5.0,
    ) -> Dict[str, Any]:
        total = len(values)
        if total == 0:
            return {"healthy": True, "missing_pct": 0.0, "total": 0}

        null_count = sum(1 for v in values if v is None or v == "")
        missing_pct = (null_count / total) * 100.0

        numeric_vals = [float(v) for v in values if isinstance(v, (int, float))]
        mean_val = (sum(numeric_vals) / len(numeric_vals)) if numeric_vals else 0.0

        return {
            "feature_name": feature_name,
            "total_records": total,
            "missing_count": null_count,
            "missing_pct": round(missing_pct, 2),
            "mean_value": round(mean_val, 4),
            "healthy": missing_pct <= max_missing_pct,
        }
