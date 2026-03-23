# Schema Reference

## TableConfig

`TableConfig` is the bridge between your Pydantic model and the Arrow/Delta Lake storage layer. It eagerly converts the model's JSON Schema to an Arrow schema, computes a fingerprint, and validates the namespace — all at construction time.

### Constructor

```python
from scouter.bifrost import TableConfig

config = TableConfig(
    model=MyModel,
    catalog="production",
    schema_name="ml",
    table="predictions",
    partition_columns=["model_version"],
)
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `model` | `Type[BaseModel]` | Yes | Pydantic model **class** (not an instance) |
| `catalog` | `str` | Yes | Top-level namespace (e.g., `"production"`, `"staging"`) |
| `schema_name` | `str` | Yes | Schema namespace (e.g., `"ml"`, `"analytics"`) |
| `table` | `str` | Yes | Table name (e.g., `"predictions"`) |
| `partition_columns` | `List[str]` | No | Additional partition columns beyond `scouter_partition_date` |

**Validation rules:**

- `catalog`, `schema_name`, and `table` must be non-empty
- No `/` or `..` characters allowed (prevents path traversal)
- Model field names must not collide with system columns (`scouter_created_at`, `scouter_partition_date`, `scouter_batch_id`)

### Properties

```python
config.catalog             # "production"
config.schema_name         # "ml"
config.table               # "predictions"
config.partition_columns   # ["model_version"]
config.fqn                 # "production.ml.predictions"
config.fingerprint_str     # "a1b2c3d4e5f6..." (32-char hex)
```

## Static Utility Methods

`TableConfig` exposes two static methods for inspecting schemas without creating a full config.

### parse_schema

```python
fields = TableConfig.parse_schema(MyModel.model_json_schema())
```

Returns a `Dict[str, Dict[str, Any]]` mapping each field name to its Arrow type and nullability:

```python
{
    "user_id": {"arrow_type": "Utf8View", "nullable": False},
    "prediction": {"arrow_type": "Float64", "nullable": False},
    "label": {"arrow_type": "Utf8View", "nullable": True},
    "scouter_created_at": {"arrow_type": "Timestamp(Microsecond, Some(\"UTC\"))", "nullable": False},
    "scouter_partition_date": {"arrow_type": "Date32", "nullable": False},
    "scouter_batch_id": {"arrow_type": "Utf8", "nullable": False},
}
```

System columns are included in the output. Use this to verify how your Pydantic types map to Arrow before pushing data.

### compute_fingerprint

```python
fp = TableConfig.compute_fingerprint(MyModel.model_json_schema())
```

Returns a 32-character hexadecimal string (SHA-256 truncated). Properties:

- **Deterministic**: Same schema always produces the same fingerprint
- **Field-order-independent**: Reordering fields in the model does not change the fingerprint
- **Sensitive to changes**: Adding, removing, or changing the type of any field produces a different fingerprint

## Type Mapping

The schema conversion follows these rules when translating Pydantic JSON Schema types to Arrow:

### Primitive types

| Python type | JSON Schema | Arrow type |
|-------------|-------------|------------|
| `str` | `{"type": "string"}` | `Utf8View` |
| `int` | `{"type": "integer"}` | `Int64` |
| `float` | `{"type": "number"}` | `Float64` |
| `bool` | `{"type": "boolean"}` | `Boolean` |

### Temporal types

| Python type | JSON Schema | Arrow type |
|-------------|-------------|------------|
| `datetime` | `{"type": "string", "format": "date-time"}` | `Timestamp(Microsecond, UTC)` |
| `date` | `{"type": "string", "format": "date"}` | `Date32` |

### Collection types

| Python type | JSON Schema | Arrow type |
|-------------|-------------|------------|
| `List[T]` | `{"type": "array", "items": {...}}` | `List(T)` |
| `Optional[T]` | `{"anyOf": [{T}, {"type": "null"}]}` | nullable `T` |

### Enum types

| Python type | JSON Schema | Arrow type |
|-------------|-------------|------------|
| `Enum(str)` | `{"enum": ["a", "b", "c"]}` | `Dictionary(Int16, Utf8)` |

Dictionary encoding is applied automatically for string enums — this gives significant compression for columns with repeated values.

### Nested models

| Python type | JSON Schema | Arrow type |
|-------------|-------------|------------|
| `BaseModel` subclass | `{"$ref": "#/$defs/ModelName"}` | `Struct(field1: T1, field2: T2, ...)` |

Nested models are resolved recursively via `$defs` references, up to 32 levels deep. Each nested model becomes an Arrow `Struct` with its own typed fields.

```python
class Address(BaseModel):
    street: str
    city: str
    zip_code: str

class Customer(BaseModel):
    name: str
    address: Address       # → Struct(street: Utf8View, city: Utf8View, zip_code: Utf8View)
    orders: List[Address]  # → List(Struct(...))
```

## Fingerprinting

The fingerprint is the primary mechanism for schema version tracking. It is computed as:

1. Parse the Pydantic JSON Schema string
2. Convert to Arrow schema (applying the type mapping above)
3. Inject system columns
4. Sort fields alphabetically by name
5. Compute SHA-256 over the canonical representation
6. Truncate to 32 hex characters

### Schema evolution

The current design uses **strict schema matching**. If the fingerprint of the data being written doesn't match the registered fingerprint for the table, the write is rejected.

This means:

- **Adding a field** → new fingerprint → requires a new table or re-registration
- **Removing a field** → new fingerprint → same
- **Changing a type** (e.g., `int` → `float`) → new fingerprint → same
- **Reordering fields** → same fingerprint → no change needed

!!! note
    Schema evolution (additive column changes, type widening) is planned for a future release. The fingerprinting infrastructure is designed to support it.
