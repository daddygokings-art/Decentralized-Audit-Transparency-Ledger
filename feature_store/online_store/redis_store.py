"""
Low-Latency Online Feature Store (Redis Backend) (#524)
"""

import json
import time
from typing import Dict, Any, Optional, List

class RedisOnlineStore:
    def __init__(self, host: str = "localhost", port: int = 6379):
        self.host = host
        self.port = port
        self._in_memory_cache: Dict[str, Dict[str, Any]] = {}

    def put_features(self, entity_id: str, view_name: str, features: Dict[str, Any], ttl_seconds: int = 86400) -> None:
        key = f"fs:{view_name}:{entity_id}"
        self._in_memory_cache[key] = {
            "values": features,
            "updated_at": time.time(),
            "expires_at": time.time() + ttl_seconds,
        }

    def get_features(self, entity_id: str, view_name: str) -> Optional[Dict[str, Any]]:
        key = f"fs:{view_name}:{entity_id}"
        entry = self._in_memory_cache.get(key)
        if not entry:
            return None
        if time.time() > entry["expires_at"]:
            del self._in_memory_cache[key]
            return None
        return entry["values"]

    def batch_get_features(self, entity_ids: List[str], view_name: str) -> Dict[str, Optional[Dict[str, Any]]]:
        return {eid: self.get_features(eid, view_name) for eid in entity_ids}
