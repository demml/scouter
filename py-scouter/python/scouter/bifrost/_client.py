from __future__ import annotations

from typing import Any, Dict, List, Optional

from .._scouter import DatasetClient, DatasetProducer, TableConfig, WriteConfig


class Bifrost:
    """Unified read/write client for the Bifrost dataset engine.

    Wraps both ``DatasetProducer`` and ``DatasetClient`` into a single object.
    Use this when you need both write and read access to the same table.
    Access the underlying clients directly via ``.producer`` and ``.client``
    for the full API.

    Args:
        table_config: Table configuration derived from a Pydantic model.
        transport: gRPC transport configuration (``GrpcConfig`` instance).
        write_config: Optional write configuration for batching behavior.
    """

    def __init__(
        self,
        table_config: TableConfig,
        transport: Any,
        write_config: Optional[WriteConfig] = None,
    ) -> None:
        self._producer = DatasetProducer(
            table_config=table_config,
            transport=transport,
            write_config=write_config,
        )
        self._client = DatasetClient(transport=transport, table_config=table_config)

    # --- Write ---

    def insert(self, record: Any) -> None:
        """Insert a Pydantic model instance into the queue. Non-blocking."""
        self._producer.insert(record)

    def flush(self) -> None:
        """Signal the background queue to flush immediately."""
        self._producer.flush()

    def shutdown(self) -> None:
        """Gracefully shut down the producer, flushing remaining items."""
        self._producer.shutdown()

    def register(self) -> str:
        """Register the dataset table with the server.

        Optional — auto-registers on first flush if not called explicitly.
        """
        return self._producer.register()

    @property
    def fingerprint(self) -> str:
        """Schema fingerprint as a 32-character hex string."""
        return self._producer.fingerprint

    @property
    def namespace(self) -> str:
        """Fully-qualified table name (``catalog.schema_name.table``)."""
        return self._producer.namespace

    @property
    def is_registered(self) -> bool:
        """Whether the dataset has been registered with the server."""
        return self._producer.is_registered

    # --- Read ---

    def read(self, limit: Optional[int] = None) -> List[Any]:
        """Read rows from the bound table as validated Pydantic model instances."""
        return self._client.read(limit=limit)

    def sql(self, query: str) -> Any:
        """Execute a SQL SELECT query and return a ``QueryResult``."""
        return self._client.sql(query)

    def list_datasets(self) -> List[Dict[str, Any]]:
        """List all registered datasets on the server."""
        return self._client.list_datasets()

    def describe_dataset(self, catalog: str, schema_name: str, table: str) -> Dict[str, Any]:
        """Get metadata and schema for a specific dataset."""
        return self._client.describe_dataset(catalog, schema_name, table)

    # --- Advanced access ---

    @property
    def producer(self) -> DatasetProducer:
        """The underlying ``DatasetProducer`` for full write API access."""
        return self._producer

    @property
    def client(self) -> DatasetClient:
        """The underlying ``DatasetClient`` for full read API access."""
        return self._client
