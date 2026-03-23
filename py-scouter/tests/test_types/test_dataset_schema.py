"""Validates that Pydantic model schemas are correctly parsed into Arrow field descriptors."""

from datetime import date, datetime
from enum import Enum
from typing import List, Optional

import pytest
from pydantic import BaseModel
from scouter.dataset import TableConfig

# ── Model definitions ─────────────────────────────────────────────────────────


class FlatModel(BaseModel):
    user_id: str
    event_type: str
    value: float
    count: int
    active: bool
    label: str


class OptionalModel(BaseModel):
    name: str
    age: Optional[int] = None
    score: Optional[float] = None


class Address(BaseModel):
    street: str
    city: str
    zip_code: str


class OrderModel(BaseModel):
    order_id: str
    address: Address


class DateTimeModel(BaseModel):
    created_at: datetime
    event_date: date
    label: str


class ListModel(BaseModel):
    model_id: str
    scores: List[float]


class Status(str, Enum):
    active = "active"
    inactive = "inactive"
    pending = "pending"


class EnumModel(BaseModel):
    status: Status
    name: str


class ReportItem(BaseModel):
    label: str
    value: float


class ReportModel(BaseModel):
    report_id: str
    items: List[ReportItem]


class DeepModel(BaseModel):
    id: str
    optional_address: Optional[Address] = None


SYSTEM_COLUMNS = {"scouter_created_at", "scouter_partition_date", "scouter_batch_id"}

# ── Flat scalar types ─────────────────────────────────────────────────────────


def test_string_maps_to_utf8view() -> None:
    fields = TableConfig.parse_schema(FlatModel.model_json_schema())
    assert fields["user_id"]["arrow_type"] == "Utf8View"
    assert fields["user_id"]["nullable"] is False


def test_float_maps_to_float64() -> None:
    fields = TableConfig.parse_schema(FlatModel.model_json_schema())
    assert fields["value"]["arrow_type"] == "Float64"
    assert fields["value"]["nullable"] is False


def test_int_maps_to_int64() -> None:
    fields = TableConfig.parse_schema(FlatModel.model_json_schema())
    assert fields["count"]["arrow_type"] == "Int64"


def test_bool_maps_to_boolean() -> None:
    fields = TableConfig.parse_schema(FlatModel.model_json_schema())
    assert fields["active"]["arrow_type"] == "Boolean"


# ── System columns ────────────────────────────────────────────────────────────


def test_system_columns_present() -> None:
    fields = TableConfig.parse_schema(FlatModel.model_json_schema())
    assert SYSTEM_COLUMNS.issubset(fields.keys())


def test_system_columns_not_nullable() -> None:
    fields = TableConfig.parse_schema(FlatModel.model_json_schema())
    assert fields["scouter_created_at"]["nullable"] is False
    assert fields["scouter_partition_date"]["nullable"] is False
    assert fields["scouter_batch_id"]["nullable"] is False


def test_system_column_types() -> None:
    fields = TableConfig.parse_schema(FlatModel.model_json_schema())
    assert "Timestamp" in fields["scouter_created_at"]["arrow_type"]
    assert "UTC" in fields["scouter_created_at"]["arrow_type"]
    assert fields["scouter_partition_date"]["arrow_type"] == "Date32"
    assert fields["scouter_batch_id"]["arrow_type"] == "Utf8"


# ── Optional fields ───────────────────────────────────────────────────────────


def test_required_field_not_nullable() -> None:
    fields = TableConfig.parse_schema(OptionalModel.model_json_schema())
    assert fields["name"]["nullable"] is False


def test_optional_int_is_nullable() -> None:
    fields = TableConfig.parse_schema(OptionalModel.model_json_schema())
    assert fields["age"]["nullable"] is True
    assert fields["age"]["arrow_type"] == "Int64"


def test_optional_float_is_nullable() -> None:
    fields = TableConfig.parse_schema(OptionalModel.model_json_schema())
    assert fields["score"]["nullable"] is True
    assert fields["score"]["arrow_type"] == "Float64"


# ── Nested model ──────────────────────────────────────────────────────────────


def test_nested_struct_type() -> None:
    fields = TableConfig.parse_schema(OrderModel.model_json_schema())
    assert "Struct" in fields["address"]["arrow_type"]


def test_nested_not_nullable() -> None:
    fields = TableConfig.parse_schema(OrderModel.model_json_schema())
    assert fields["address"]["nullable"] is False


# ── Date/time types ───────────────────────────────────────────────────────────


def test_datetime_maps_to_timestamp() -> None:
    fields = TableConfig.parse_schema(DateTimeModel.model_json_schema())
    assert "Timestamp" in fields["created_at"]["arrow_type"]
    assert "UTC" in fields["created_at"]["arrow_type"]


def test_date_maps_to_date32() -> None:
    fields = TableConfig.parse_schema(DateTimeModel.model_json_schema())
    assert fields["event_date"]["arrow_type"] == "Date32"


# ── List types ────────────────────────────────────────────────────────────────


def test_list_of_floats() -> None:
    fields = TableConfig.parse_schema(ListModel.model_json_schema())
    assert "List" in fields["scores"]["arrow_type"]
    assert fields["scores"]["nullable"] is False


# ── Enum ──────────────────────────────────────────────────────────────────────


def test_enum_maps_to_dictionary() -> None:
    fields = TableConfig.parse_schema(EnumModel.model_json_schema())
    assert "Dictionary" in fields["status"]["arrow_type"]


# ── List of nested model ──────────────────────────────────────────────────────


def test_list_of_structs() -> None:
    fields = TableConfig.parse_schema(ReportModel.model_json_schema())
    assert "List" in fields["items"]["arrow_type"]


# ── Optional nested model ─────────────────────────────────────────────────────


def test_optional_nested_struct_is_nullable() -> None:
    fields = TableConfig.parse_schema(DeepModel.model_json_schema())
    assert "Struct" in fields["optional_address"]["arrow_type"]
    assert fields["optional_address"]["nullable"] is True


# ── Fingerprint ───────────────────────────────────────────────────────────────


def test_fingerprint_is_32_chars() -> None:
    fp = TableConfig.compute_fingerprint(FlatModel.model_json_schema())
    assert len(fp) == 32


def test_fingerprint_stable() -> None:
    assert TableConfig.compute_fingerprint(FlatModel.model_json_schema()) == TableConfig.compute_fingerprint(
        FlatModel.model_json_schema()
    )


def test_fingerprint_changes_on_field_add() -> None:
    class Extended(BaseModel):
        user_id: str
        event_type: str
        value: float
        count: int
        active: bool
        label: str
        new_field: str

    assert TableConfig.compute_fingerprint(FlatModel.model_json_schema()) != TableConfig.compute_fingerprint(
        Extended.model_json_schema()
    )


def test_fingerprint_changes_on_type_change() -> None:
    class IntLabel(BaseModel):
        user_id: str
        event_type: str
        value: float
        count: int
        active: bool
        label: int

    assert TableConfig.compute_fingerprint(FlatModel.model_json_schema()) != TableConfig.compute_fingerprint(
        IntLabel.model_json_schema()
    )


def test_fingerprint_differs_across_models() -> None:
    assert TableConfig.compute_fingerprint(FlatModel.model_json_schema()) != TableConfig.compute_fingerprint(
        OptionalModel.model_json_schema()
    )


# ── Error paths ───────────────────────────────────────────────────────────────


def test_parse_schema_unsupported_type_raises() -> None:
    schema = {
        "type": "object",
        "properties": {"x": {"type": "unknown_type"}},
        "required": ["x"],
    }
    with pytest.raises(RuntimeError, match="Unsupported"):
        TableConfig.parse_schema(schema)


def test_parse_schema_missing_ref_raises() -> None:
    schema = {
        "type": "object",
        "properties": {"x": {"$ref": "#/$defs/DoesNotExist"}},
        "required": ["x"],
    }
    with pytest.raises(RuntimeError):
        TableConfig.parse_schema(schema)


def test_parse_schema_missing_properties_raises() -> None:
    with pytest.raises(RuntimeError):
        TableConfig.parse_schema({"type": "object"})


def test_parse_schema_reserved_column_collision_raises() -> None:
    schema = {
        "type": "object",
        "properties": {"scouter_created_at": {"type": "string"}},
        "required": ["scouter_created_at"],
    }
    with pytest.raises(RuntimeError, match="reserved"):
        TableConfig.parse_schema(schema)


def test_compute_fingerprint_invalid_schema_raises() -> None:
    with pytest.raises(RuntimeError):
        TableConfig.compute_fingerprint({"no_properties": True})
