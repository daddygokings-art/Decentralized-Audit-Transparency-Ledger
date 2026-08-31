"""AuditLedger Python SDK — event validation schemas (#240).

Provides JSON Schema support, a schema registry with caching, and
helpers for validating event metadata before submission.
"""

from __future__ import annotations

import json
from typing import Any, Dict, Optional


# ---------------------------------------------------------------------------
# Exceptions
# ---------------------------------------------------------------------------


class SchemaValidationError(Exception):
    """Raised when event metadata fails schema validation."""

    def __init__(self, message: str, errors: Optional[list] = None):
        super().__init__(message)
        self.errors = errors or []


class SchemaNotFoundError(KeyError):
    """Raised when a requested schema name is not in the registry."""


# ---------------------------------------------------------------------------
# Built-in base schema
# ---------------------------------------------------------------------------

#: Minimal JSON Schema that every audit event must satisfy.
BASE_EVENT_SCHEMA: Dict[str, Any] = {
    "$schema": "http://json-schema.org/draft-07/schema#",
    "type": "object",
    "required": ["event_type", "submitter"],
    "properties": {
        "event_type": {"type": "string", "minLength": 1},
        "submitter": {"type": "string", "minLength": 1},
        "metadata": {
            "oneOf": [
                {"type": "string"},
                {"type": "object"},
                {"type": "null"},
            ]
        },
    },
    "additionalProperties": True,
}


# ---------------------------------------------------------------------------
# Schema registry
# ---------------------------------------------------------------------------


class SchemaRegistry:
    """In-memory schema registry with per-event-type versioning and migration support."""

    _JSONSCHEMA_AVAILABLE: Optional[bool] = None

    def __init__(self) -> None:
        self._schemas: Dict[str, Dict[str, Any]] = {}
        self._schema_versions: Dict[str, list[int]] = {}
        self._migrations: Dict[tuple[str, int, int], Dict[str, Any]] = {}
        self._validator_cache: Dict[str, Any] = {}
        self.register("__base__", BASE_EVENT_SCHEMA)

    def register_schema(self, event_type: str, schema: Dict[str, Any], version: int = 1) -> None:
        """Register a versioned schema for an event type."""
        if not isinstance(schema, dict):
            raise TypeError(f"Schema must be a dict, got {type(schema).__name__}")
        if version <= 0:
            raise ValueError("Schema version must be positive")
        self._schemas[f"{event_type}:{version}"] = schema
        versions = self._schema_versions.setdefault(event_type, [])
        if version not in versions:
            versions.append(version)
            versions.sort()
        self._validator_cache.pop(f"{event_type}:{version}", None)

    def get_schema(self, event_type: str, version: int) -> Dict[str, Any]:
        key = f"{event_type}:{version}"
        try:
            return self._schemas[key]
        except KeyError as exc:
            raise SchemaNotFoundError(f"No schema registered for event type {event_type!r} version {version!r}") from exc

    def list_schemas(self, event_type: Optional[str] = None) -> list[int]:
        if event_type is None:
            versions: list[int] = []
            for evt in self._schema_versions:
                versions.extend(self._schema_versions[evt])
            return sorted(set(versions))
        return list(self._schema_versions.get(event_type, []))

    def register(self, name: str, schema: Dict[str, Any]) -> None:
        """Compatibility alias for simple schema registration using a single name."""
        self._schemas[name] = schema
        self._validator_cache.pop(name, None)

    def get(self, name: str) -> Dict[str, Any]:
        try:
            return self._schemas[name]
        except KeyError:
            raise SchemaNotFoundError(f"No schema registered with name: {name!r}")

    def remove(self, name: str) -> None:
        self._schemas.pop(name, None)
        self._validator_cache.pop(name, None)

    # ---- registry CRUD -------------------------------------------------

    def register(self, name: str, schema: Dict[str, Any]) -> None:
        """Register (or replace) a schema under *name*.

        Args:
            name: Unique schema identifier.
            schema: JSON Schema dict.

        Raises:
            TypeError: If *schema* is not a dict.
        """
        if not isinstance(schema, dict):
            raise TypeError(f"Schema must be a dict, got {type(schema).__name__}")
        self._schemas[name] = schema
        # Invalidate cached validator so it is re-compiled on next use
        self._validator_cache.pop(name, None)

    def get(self, name: str) -> Dict[str, Any]:
        """Return the schema registered under *name*.

        Raises:
            SchemaNotFoundError: If no schema is registered with that name.
        """
        try:
            return self._schemas[name]
        except KeyError:
            raise SchemaNotFoundError(f"No schema registered with name: {name!r}")

    def remove(self, name: str) -> None:
        """Remove the schema registered under *name* (no-op if not present)."""
        self._schemas.pop(name, None)
        self._validator_cache.pop(name, None)

    def list_schemas(self) -> list:
        """Return the names of all registered schemas (sorted)."""
        return sorted(self._schemas.keys())

    def has(self, name: str) -> bool:
        """Return True if a schema named *name* is registered."""
        return name in self._schemas

    # ---- validation ----------------------------------------------------

    def validate(self, name: str, data: Any) -> None:
        """Validate *data* against the schema registered under *name*.

        Args:
            name: Schema name previously passed to :meth:`register`.
            data: Data to validate — typically a dict or JSON-decoded object.

        Raises:
            SchemaNotFoundError: If the schema is not registered.
            SchemaValidationError: If *data* does not conform to the schema.
        """
        schema = self.get(name)
        self._validate_against_schema(schema, data, schema_name=name)

    def validate_event_metadata(
        self, event_type: str, metadata: Any, *, fallback_to_base: bool = True, version: Optional[int] = None
    ) -> None:
        """Validate *metadata* for the given *event_type*.

        If *version* is provided the schema for that version is used. Otherwise,
        the latest registered version for this event type is selected, and if no
        event-specific schema exists the base schema is used as fallback.
        """
        if version is not None:
            schema_name = f"{event_type}:{version}"
            if self._schemas.get(schema_name) is not None:
                self._validate_against_schema(self._schemas[schema_name], metadata, schema_name=schema_name)
                return
            if self._schemas.get(event_type) is not None:
                self._validate_against_schema(self._schemas[event_type], metadata, schema_name=event_type)
                return
            if not fallback_to_base:
                raise SchemaNotFoundError(f"No schema registered for event type: {event_type!r} version {version!r}")

        schema_direct = self._schemas.get(event_type)
        if schema_direct is not None:
            self._validate_against_schema(schema_direct, metadata, schema_name=event_type)
            return

        latest_version = None
        if self._schema_versions.get(event_type):
            latest_version = max(self._schema_versions[event_type])
        if latest_version is not None:
            self.validate_event_metadata(event_type, metadata, fallback_to_base=False, version=latest_version)
            return
        if fallback_to_base:
            self._validate_against_schema(BASE_EVENT_SCHEMA, metadata, schema_name="__base__")
            return
        raise SchemaNotFoundError(f"No schema registered for event type: {event_type!r}")

    def migrate_event_metadata(self, event_type: str, data: Any, from_version: int, to_version: int) -> Any:
        """Apply a registered migration between schema versions."""
        key = (event_type, from_version, to_version)
        migration = self._migrations.get(key)
        if migration is None:
            raise SchemaNotFoundError(f"No migration registered for {event_type!r} from {from_version} to {to_version}")
        fn = migration.get("fn")
        if fn is None:
            return data
        return fn(data)

    def register_migration(self, event_type: str, from_version: int, to_version: int, fn) -> None:
        """Register a migration function for a schema version transition."""
        self._migrations[(event_type, from_version, to_version)] = {"fn": fn}

    @staticmethod
    def check_compatibility(old_schema: Dict[str, Any], new_schema: Dict[str, Any]) -> str:
        """Return compatibility classification for a schema evolution."""
        old_required = set(old_schema.get("required", []))
        new_required = set(new_schema.get("required", []))
        if old_required == new_required:
            return "full"
        if old_required.issubset(new_required):
            return "backward"
        if new_required.issubset(old_required):
            return "forward"
        return "breaking"

    # ---- internals -----------------------------------------------------

    @classmethod
    def _jsonschema_available(cls) -> bool:
        if cls._JSONSCHEMA_AVAILABLE is None:
            try:
                import jsonschema  # noqa: F401

                cls._JSONSCHEMA_AVAILABLE = True
            except ImportError:
                cls._JSONSCHEMA_AVAILABLE = False
        return cls._JSONSCHEMA_AVAILABLE

    def _get_validator(self, name: str, schema: Dict[str, Any]) -> Any:
        """Return (and cache) a compiled jsonschema validator."""
        if name not in self._validator_cache:
            import jsonschema

            self._validator_cache[name] = jsonschema.Draft7Validator(schema)
        return self._validator_cache[name]

    def _validate_against_schema(
        self, schema: Dict[str, Any], data: Any, *, schema_name: str = ""
    ) -> None:
        """Run validation, raising SchemaValidationError on failure.

        Falls back to a lightweight built-in check when *jsonschema* is not
        installed.
        """
        if self._jsonschema_available():
            import jsonschema

            validator = self._get_validator(schema_name, schema)
            errors = list(validator.iter_errors(data))
            if errors:
                messages = [e.message for e in errors]
                raise SchemaValidationError(
                    f"Schema validation failed ({schema_name!r}): "
                    + "; ".join(messages),
                    errors=messages,
                )
        else:
            # Lightweight fallback — check required fields and basic types
            self._builtin_validate(schema, data, schema_name)

    @staticmethod
    def _builtin_validate(
        schema: Dict[str, Any], data: Any, schema_name: str
    ) -> None:
        """Minimal JSON Schema validator used when *jsonschema* is absent."""
        errors: list[str] = []

        # type check
        expected_type = schema.get("type")
        type_map = {
            "object": dict,
            "array": list,
            "string": str,
            "number": (int, float),
            "integer": int,
            "boolean": bool,
            "null": type(None),
        }
        if expected_type and expected_type in type_map:
            if not isinstance(data, type_map[expected_type]):
                errors.append(
                    f"Expected type {expected_type!r}, "
                    f"got {type(data).__name__!r}"
                )

        # required fields
        if isinstance(data, dict):
            for field in schema.get("required", []):
                if field not in data:
                    errors.append(f"Required field {field!r} is missing")

            # property-level type checks
            for prop, prop_schema in schema.get("properties", {}).items():
                if prop in data and isinstance(prop_schema, dict):
                    prop_type = prop_schema.get("type")
                    if prop_type and prop_type in type_map:
                        if not isinstance(data[prop], type_map[prop_type]):
                            errors.append(
                                f"Property {prop!r}: expected {prop_type!r}, "
                                f"got {type(data[prop]).__name__!r}"
                            )
                    # minLength for strings
                    if prop_type == "string" and "minLength" in prop_schema:
                        if len(data.get(prop, "")) < prop_schema["minLength"]:
                            errors.append(
                                f"Property {prop!r} must have length >= "
                                f"{prop_schema['minLength']}"
                            )

        if errors:
            raise SchemaValidationError(
                f"Schema validation failed ({schema_name!r}): "
                + "; ".join(errors),
                errors=errors,
            )


# ---------------------------------------------------------------------------
# Module-level default registry
# ---------------------------------------------------------------------------

#: Default global registry — used by :func:`validate_event`.
_default_registry = SchemaRegistry()


def get_default_registry() -> SchemaRegistry:
    """Return the module-level default :class:`SchemaRegistry`."""
    return _default_registry


def validate_event(event_type: str, metadata: Any) -> None:
    """Validate *metadata* using the default global registry.

    Convenience wrapper around :meth:`SchemaRegistry.validate_event_metadata`.

    Raises:
        SchemaValidationError: If *metadata* does not conform to the schema.
    """
    _default_registry.validate_event_metadata(event_type, metadata)
