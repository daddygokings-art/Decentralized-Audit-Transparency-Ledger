"""
Feature Drift & Distribution Shift Detector (#524)
"""

import math
from typing import List, Dict, Any, Tuple

class DriftDetector:
    @staticmethod
    def calculate_psi(baseline: List[float], current: List[float], bins: int = 10) -> float:
        """
        Calculates Population Stability Index (PSI) between baseline and current distributions.
        PSI < 0.1: No significant change
        0.1 <= PSI < 0.2: Moderate change
        PSI >= 0.2: Significant drift detected
        """
        if not baseline or not current:
            return 0.0

        min_val = min(min(baseline), min(current))
        max_val = max(max(baseline), max(current))

        if min_val == max_val:
            return 0.0

        bin_width = (max_val - min_val) / bins
        psi = 0.0

        for i in range(bins):
            b_low = min_val + i * bin_width
            b_high = b_low + bin_width

            # Count proportions
            b_count = sum(1 for x in baseline if b_low <= x < b_high or (i == bins - 1 and x == b_high))
            c_count = sum(1 for x in current if b_low <= x < b_high or (i == bins - 1 and x == b_high))

            b_pct = max(b_count / len(baseline), 0.0001)
            c_pct = max(c_count / len(current), 0.0001)

            psi += (c_pct - b_pct) * math.log(c_pct / b_pct)

        return float(psi)

    @staticmethod
    def check_feature_drift(
        feature_name: str,
        baseline: List[float],
        current: List[float],
        threshold: float = 0.2,
    ) -> Tuple[bool, float]:
        psi = DriftDetector.calculate_psi(baseline, current)
        is_drifting = psi >= threshold
        return is_drifting, psi
