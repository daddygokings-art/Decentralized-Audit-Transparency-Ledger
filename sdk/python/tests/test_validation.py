"""Tests for SDK event validation schemas (#240)."""

from __future__ import annotations

import pytest

from audit_ledger.validation import (
    SchemaRegistry,
    SchemaValidationError,
    SchemaNotFoundError,
    get_default_registry,
    validate_event,
    BASE_EVENT_SCHEMA,
)


# ── SchemaRegistry CRUD ───────────────────────────────────────────────────────

class TestSchemaRegistryCRUD:
    def test_register_and_get(self):
        reg = SchemaRegistry()
        schema = {"type": "object", "required": ["amount"]}
        reg.register("payment", schema)
        assert reg.get("payment") == schema

    def test_get_missing_raises(self):
        reg = SchemaRegistry()
        with pytest.raises(SchemaNotFoundError):
            reg.get("nonexistent")

    def test_remove_schema(self):
        reg = SchemaRegistry()
        reg.register("tmp", {"type": "object"})
        reg.remove("tmp")
        assert not reg.has("tmp")

    def test_remove_nonexistent_is_noop(self):
        reg = SchemaRegistry()
        reg.remove("does_not_exist")  # should not raise

    def test_list_schemas_sorted(self):
        reg = SchemaRegistry()
        reg.register("zschema", {"type": "object"})
        reg.register("aschema", {"type": "object"})
        names = reg.list_schemas()
        assert names == sorted(names)
        assert "aschema" in names
        assert "zschema" in names

    def test_has_returns_true_for_registered(self):
        reg = SchemaRegistry()
        reg.register("exists", {"type": "object"})
        assert reg.has("exists") is True

    def test_has_returns_false_for_missing(self):
        reg = SchemaRegistry()
        assert reg.has("missing") is False

    def test_register_replaces_existing(self):
        reg = SchemaRegistry()
        reg.register("s", {"type": "object"})
        reg.register("s", {"type": "string"})
        assert reg.get("s")["type"] == "string"

    def test_register_non_dict_raises(self):
        reg = SchemaRegistry()
        with pytest.raises(TypeError):
            reg.register("bad", "not-a-dict")

    def test_base_schema_preregistered(self):
        reg = SchemaRegistry()
        assert reg.has("__base__")
        assert reg.get("__base__") == BASE_EVENT_SCHEMA


# ── Validation — builtin fallback (no jsonschema) ─────────────────────────────

class TestBuiltinValidation:
    """These tests use the _builtin_validate path directly."""

    def _validate(self, schema, data):
        SchemaRegistry._builtin_validate(schema, data, "test")

    def test_valid_object(self):
        self._validate(
            {"type": "object", "required": ["name"], "properties": {"name": {"type": "string"}}},
            {"name": "Alice"},
        )

    def test_missing_required_field(self):
        with pytest.raises(SchemaValidationError, match="Required field"):
            self._validate({"type": "object", "required": ["amount"]}, {})

    def test_wrong_type(self):
        with pytest.raises(SchemaValidationError, match="Expected type"):
            self._validate({"type": "object"}, "not-an-object")

    def test_property_wrong_type(self):
        with pytest.raises(SchemaValidationError, match="expected 'string'"):
            self._validate(
                {"type": "object", "properties": {"name": {"type": "string"}}},
                {"name": 123},
            )

    def test_min_length_violation(self):
        with pytest.raises(SchemaValidationError, match="length"):
            self._validate(
                {"type": "object", "properties": {"code": {"type": "string", "minLength": 3}}},
                {"code": "ab"},
            )

    def test_valid_min_length(self):
        self._validate(
            {"type": "object", "properties": {"code": {"type": "string", "minLength": 2}}},
            {"code": "ab"},
        )


# ── SchemaRegistry.validate ───────────────────────────────────────────────────

class TestSchemaRegistryValidate:
    def test_validate_valid_data(self):
        reg = SchemaRegistry()
        reg.register("payment", {
            "type": "object",
            "required": ["amount"],
            "properties": {"amount": {"type": "number"}},
        })
        reg.validate("payment", {"amount": 100})

    def test_validate_invalid_data_raises(self):
        reg = SchemaRegistry()
        reg.register("payment", {
            "type": "object",
            "required": ["amount"],
        })
        with pytest.raises(SchemaValidationError):
            reg.validate("payment", {})

    def test_validate_missing_schema_raises(self):
        reg = SchemaRegistry()
        with pytest.raises(SchemaNotFoundError):
            reg.validate("nonexistent", {})


# ── validate_event_metadata ───────────────────────────────────────────────────

class TestValidateEventMetadata:
    def test_falls_back_to_base_schema(self):
        reg = SchemaRegistry()
        # base schema requires event_type and submitter
        reg.validate_event_metadata(
            "unknown_type",
            {"event_type": "unknown_type", "submitter": "GABC"},
        )

    def test_uses_registered_schema_when_present(self):
        reg = SchemaRegistry()
        reg.register("payment", {
            "type": "object",
            "required": ["amount"],
            "properties": {"amount": {"type": "number"}},
        })
        reg.validate_event_metadata("payment", {"amount": 50})

    def test_raises_when_no_schema_and_fallback_disabled(self):
        reg = SchemaRegistry()
        with pytest.raises(SchemaNotFoundError):
            reg.validate_event_metadata(
                "no_schema_type", {}, fallback_to_base=False
            )

    def test_registered_schema_validation_fails(self):
        reg = SchemaRegistry()
        reg.register("payment", {
            "type": "object",
            "required": ["amount"],
        })
        with pytest.raises(SchemaValidationError):
            reg.validate_event_metadata("payment", {"no_amount": True})


# ── Caching ───────────────────────────────────────────────────────────────────

class TestSchemaCache:
    def test_re_register_invalidates_cache(self):
        reg = SchemaRegistry()
        reg.register("s", {"type": "object", "required": ["a"]})
        # Warm up cache by validating
        try:
            reg.validate("s", {"a": 1})
        except SchemaValidationError:
            pass
        # Re-register without the required field
        reg.register("s", {"type": "object"})
        # Should now pass (no required field)
        reg.validate("s", {})

    def test_remove_also_clears_cache(self):
        reg = SchemaRegistry()
        reg.register("s", {"type": "object"})
        reg.validate("s", {})
        reg.remove("s")
        with pytest.raises(SchemaNotFoundError):
            reg.validate("s", {})


# ── Module-level helpers ──────────────────────────────────────────────────────

class TestModuleLevelHelpers:
    def test_get_default_registry_returns_registry(self):
        reg = get_default_registry()
        assert isinstance(reg, SchemaRegistry)

    def test_get_default_registry_same_instance(self):
        assert get_default_registry() is get_default_registry()

    def test_validate_event_uses_default_registry(self):
        reg = get_default_registry()
        reg.register("__test_mod__", {
            "type": "object",
            "required": ["x"],
        })
        validate_event("__test_mod__", {"x": 1})
        # Cleanup
        reg.remove("__test_mod__")

    def test_validate_event_falls_back_to_base(self):
        # Any unknown type falls back to base schema
        validate_event("totally_unknown", {"event_type": "t", "submitter": "G"})

    def test_base_event_schema_has_required_fields(self):
        assert "required" in BASE_EVENT_SCHEMA
        assert "event_type" in BASE_EVENT_SCHEMA["required"]
        assert "submitter" in BASE_EVENT_SCHEMA["required"]
