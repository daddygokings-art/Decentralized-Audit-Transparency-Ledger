"""
Feature View Definitions (#524)
"""

from dataclasses import dataclass, field
from typing import List, Dict, Any, Optional

@dataclass
class Feature:
    name: str
    data_type: str  # float, int, string, bytes, vector
    description: str
    tags: Dict[str, str] = field(default_factory=dict)

@dataclass
class FeatureView:
    name: str
    entities: List[str]
    features: List[Feature]
    ttl_seconds: int = 86400 * 30 # 30 days default
    online_serving: bool = True
    batch_source: str = "raw_contract_events"
    streaming_source: Optional[str] = "stellar_events_stream"
    version: int = 1

    def get_feature_names(self) -> List[str]:
        return [f.name for f in self.features]
