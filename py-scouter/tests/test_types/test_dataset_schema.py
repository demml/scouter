"""Validates that Pydantic model schemas are correctly parsed into Arrow field descriptors."""

from datetime import date, datetime
from enum import Enum
from typing import List, Optional

from pydantic import BaseModel
from scouter._scouter import DatasetClient

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


def test_string_maps_to_utf8view():
    fields = DatasetClient.parse_schema(FlatModel.model_json_schema())
    assert fields["user_id"]["arrow_type"] == "Utf8View"
    assert fields["user_id"]["nullable"] is False


def test_float_maps_to_float64():
    fields = DatasetClient.parse_schema(FlatModel.model_json_schema())
    assert fields["value"]["arrow_type"] == "Float64"
    assert fields["value"]["nullable"] is False


def test_int_maps_to_int64():
    fields = DatasetClient.parse_schema(FlatModel.model_json_schema())
    assert fields["count"]["arrow_type"] == "Int64"


def test_bool_maps_to_boolean():
    fields = DatasetClient.parse_schema(FlatModel.model_json_schema())
    assert fields["active"]["arrow_type"] == "Boolean"


# ── System columns ────────────────────────────────────────────────────────────


def test_system_columns_present():
    fields = DatasetClient.parse_schema(FlatModel.model_json_schema())
    assert SYSTEM_COLUMNS.issubset(fields.keys())


def test_system_columns_not_nullable():
    fields = DatasetClient.parse_schema(FlatModel.model_json_schema())
    assert fields["scouter_created_at"]["nullable"] is False
    assert fields["scouter_partition_date"]["nullable"] is False
    assert fields["scouter_batch_id"]["nullable"] is False


def test_system_column_types():
    fields = DatasetClient.parse_schema(FlatModel.model_json_schema())
    assert "Timestamp" in fields["scouter_created_at"]["arrow_type"]
    assert "UTC" in fields["scouter_created_at"]["arrow_type"]
    assert fields["scouter_partition_date"]["arrow_type"] == "Date32"
    assert fields["scouter_batch_id"]["arrow_type"] == "Utf8"


# ── Optional fields ───────────────────────────────────────────────────────────


def test_required_field_not_nullable():
    fields = DatasetClient.parse_schema(OptionalModel.model_json_schema())
    assert fields["name"]["nullable"] is False


def test_optional_int_is_nullable():
    fields = DatasetClient.parse_schema(OptionalModel.model_json_schema())
    assert fields["age"]["nullable"] is True
    assert fields["age"]["arrow_type"] == "Int64"


def test_optional_float_is_nullable():
    fields = DatasetClient.parse_schema(OptionalModel.model_json_schema())
    assert fields["score"]["nullable"] is True
    assert fields["score"]["arrow_type"] == "Float64"


# ── Nested model ──────────────────────────────────────────────────────────────


def test_nested_struct_type():
    fields = DatasetClient.parse_schema(OrderModel.model_json_schema())
    assert "Struct" in fields["address"]["arrow_type"]


def test_nested_not_nullable():
    fields = DatasetClient.parse_schema(OrderModel.model_json_schema())
    assert fields["address"]["nullable"] is False


# ── Date/time types ───────────────────────────────────────────────────────────


def test_datetime_maps_to_timestamp():
    fields = DatasetClient.parse_schema(DateTimeModel.model_json_schema())
    assert "Timestamp" in fields["created_at"]["arrow_type"]
    assert "UTC" in fields["created_at"]["arrow_type"]


def test_date_maps_to_date32():
    fields = DatasetClient.parse_schema(DateTimeModel.model_json_schema())
    assert fields["event_date"]["arrow_type"] == "Date32"


# ── List types ────────────────────────────────────────────────────────────────


def test_list_of_floats():
    fields = DatasetClient.parse_schema(ListModel.model_json_schema())
    assert "List" in fields["scores"]["arrow_type"]
    assert fields["scores"]["nullable"] is False


# ── Enum ──────────────────────────────────────────────────────────────────────


def test_enum_maps_to_dictionary():
    fields = DatasetClient.parse_schema(EnumModel.model_json_schema())
    assert "Dictionary" in fields["status"]["arrow_type"]


# ── List of nested model ──────────────────────────────────────────────────────


def test_list_of_structs():
    fields = DatasetClient.parse_schema(ReportModel.model_json_schema())
    assert "List" in fields["items"]["arrow_type"]


# ── Optional nested model ─────────────────────────────────────────────────────


def test_optional_nested_struct_is_nullable():
    fields = DatasetClient.parse_schema(DeepModel.model_json_schema())
    assert "Struct" in fields["optional_address"]["arrow_type"]
    assert fields["optional_address"]["nullable"] is True


# ── Fingerprint ───────────────────────────────────────────────────────────────


def test_fingerprint_is_16_chars():
    fp = DatasetClient.compute_fingerprint(FlatModel.model_json_schema())
    assert len(fp) == 16


def test_fingerprint_stable():
    assert DatasetClient.compute_fingerprint(
        FlatModel.model_json_schema()
    ) == DatasetClient.compute_fingerprint(FlatModel.model_json_schema())


def test_fingerprint_changes_on_field_add():
    class Extended(BaseModel):
        user_id: str
        event_type: str
        value: float
        count: int
        active: bool
        label: str
        new_field: str

    assert DatasetClient.compute_fingerprint(
        FlatModel.model_json_schema()
    ) != DatasetClient.compute_fingerprint(Extended.model_json_schema())


def test_fingerprint_changes_on_type_change():
    class IntLabel(BaseModel):
        user_id: str
        event_type: str
        value: float
        count: int
        active: bool
        label: int

    assert DatasetClient.compute_fingerprint(
        FlatModel.model_json_schema()
    ) != DatasetClient.compute_fingerprint(IntLabel.model_json_schema())


def test_fingerprint_differs_across_models():
    assert DatasetClient.compute_fingerprint(
        FlatModel.model_json_schema()
    ) != DatasetClient.compute_fingerprint(OptionalModel.model_json_schema())
