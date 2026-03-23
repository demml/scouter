#### begin imports ####

from typing import Any, Dict, List, Optional, Type

#### end of imports ####

__all__ = ["DatasetClient", "DatasetProducer", "TableConfig", "WriteConfig"]

class DatasetClient:
    """Dataset client for reads and queries (Phase 5)."""

class TableConfig:
    """Configuration for a dataset table, derived from a Pydantic model.

    Eagerly computes Arrow schema, fingerprint, and namespace from the model class.

    Args:
        model: Pydantic model class (not an instance).
        catalog: Catalog name.
        schema_name: Schema name.
        table: Table name.
        partition_columns: Optional list of partition column names.
    """

    catalog: str
    schema_name: str
    table: str
    partition_columns: List[str]

    def __init__(
        self,
        model: Type[Any],
        catalog: str,
        schema_name: str,
        table: str,
        partition_columns: Optional[List[str]] = None,
    ) -> None: ...
    @property
    def fingerprint_str(self) -> str: ...
    @property
    def fqn(self) -> str: ...
    @staticmethod
    def parse_schema(schema: Dict[str, Any]) -> Dict[str, Dict[str, Any]]:
        """Parse a Pydantic model's JSON Schema dict into a field map.

        Accepts the dict returned directly by ``Model.model_json_schema()``.

        System columns (``scouter_created_at``, ``scouter_partition_date``,
        ``scouter_batch_id``) are included automatically.

        Args:
            schema: Dict returned by ``Model.model_json_schema()``.

        Returns:
            Mapping of field name to Arrow type descriptor
            with ``arrow_type`` (str) and ``nullable`` (bool) keys.
        """

    @staticmethod
    def compute_fingerprint(schema: Dict[str, Any]) -> str:
        """Compute a stable 32-character SHA-256 fingerprint from a JSON Schema dict.

        The fingerprint is deterministic — the same schema always yields the same value.
        Any field addition, removal, or type change yields a different value.

        Args:
            schema: Dict returned by ``Model.model_json_schema()``.

        Returns:
            32-character hexadecimal fingerprint string.
        """

class WriteConfig:
    """Configuration for dataset write behavior.

    Args:
        batch_size: Number of rows per batch (default: 1000).
        scheduled_delay_secs: Seconds between scheduled flushes (default: 30).
    """

    batch_size: int
    scheduled_delay_secs: int

    def __init__(
        self,
        batch_size: int = 1000,
        scheduled_delay_secs: int = 30,
    ) -> None: ...

class DatasetProducer:
    """Real-time streaming producer for the Scouter dataset engine.

    Pushes Pydantic model instances through a Rust queue to Delta Lake via gRPC.
    Always has an active background queue.

    Args:
        table_config: Table configuration derived from a Pydantic model.
        transport: Transport configuration (e.g., GrpcConfig).
        write_config: Optional write configuration.
    """

    def __init__(
        self,
        table_config: TableConfig,
        transport: Any,
        write_config: Optional[WriteConfig] = None,
    ) -> None: ...
    def insert(self, record: Any) -> None:
        """Insert a Pydantic model instance into the queue.

        Calls ``record.model_dump_json()`` and sends via channel. Non-blocking.
        """

    def flush(self) -> None:
        """Signal the background queue to flush immediately."""

    def shutdown(self) -> None:
        """Gracefully shut down the producer, flushing remaining items."""

    def register(self) -> str:
        """Register the dataset table with the server.

        Optional — auto-registers on first flush if not called explicitly.

        Returns:
            Registration status from the server.
        """

    @property
    def fingerprint(self) -> str: ...
    @property
    def namespace(self) -> str: ...
    @property
    def is_registered(self) -> bool: ...
