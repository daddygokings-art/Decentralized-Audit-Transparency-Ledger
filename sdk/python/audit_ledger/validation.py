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
    """In-memory schema registry with optional per-schema caching.

    Schemas are stored by name and can be retrieved, registered, updated,
    or removed at runtime.  Compiled validators (when *jsonschema* is
    available) are cached so each schema is compiled only once.

    Example::

        registry = SchemaRegistry()
        registry.register("payment", {
            "type": "object",
            "required": ["amount", "currency"],
            "properties": {
                "amount": {"type": "number", "minimum": 0},
                "currency": {"type": "string"},
            },
        })
        registry.validate("payment", {"amount": 100, "currency": "USD"})
    """

    _JSONSCHEMA_AVAILABLE: Optional[bool] = None

    def __init__(self) -> None:
        self._schemas: Dict[str, Dict[str, Any]] = {}
        # cache: schema_name -> compiled validator (or None if jsonschema unavailable)
        self._validator_cache: Dict[str, Any] = {}
        # Always pre-register the base schema
        self.register("__base__", BASE_EVENT_SCHEMA)

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
        self, event_type: str, metadata: Any, *, fallback_to_base: bool = True
    ) -> None:
        """Validate *metadata* for the given *event_type*.

        Looks up a schema named exactly *event_type*.  If no such schema is
        registered and *fallback_to_base* is True, falls back to the built-in
        base event schema.

        Args:
            event_type: The event type string (e.g. ``"payment"``).
            metadata: The decoded metadata object to validate.
            fallback_to_base: If True, use the base schema when no
                event-specific schema exists.

        Raises:
            SchemaNotFoundError: If *event_type* has no schema and
                *fallback_to_base* is False.
            SchemaValidationError: If *metadata* fails validation.
        """
        if self.has(event_type):
            self.validate(event_type, metadata)
        elif fallback_to_base:
            self._validate_against_schema(
                BASE_EVENT_SCHEMA, metadata, schema_name="__base__"
            )
        else:
            raise SchemaNotFoundError(
                f"No schema registered for event type: {event_type!r}"
            )

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
