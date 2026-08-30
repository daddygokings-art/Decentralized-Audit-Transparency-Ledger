"""
Feature Registry & Versioning (#524)
"""

from typing import Dict, Optional, List
from .feature_view import FeatureView

class FeatureRegistry:
    def __init__(self):
        self._views: Dict[str, FeatureView] = {}

    def register_view(self, view: FeatureView) -> None:
        key = f"{view.name}:v{view.version}"
        self._views[key] = view
        # Keep latest alias
        self._views[view.name] = view

    def get_view(self, name: str, version: Optional[int] = None) -> Optional[FeatureView]:
        if version is not None:
            return self._views.get(f"{name}:v{version}")
        return self._views.get(name)

    def list_views(self) -> List[FeatureView]:
        seen = set()
        result = []
        for view in self._views.values():
            if view.name not in seen:
                seen.add(view.name)
                result.append(view)
        return result
