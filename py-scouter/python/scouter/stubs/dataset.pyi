#### begin imports ####

from typing import Any, Dict

#### end of imports ####

class DatasetClient:
    @staticmethod
    def parse_schema(schema: Dict[str, Any]) -> Dict[str, Dict[str, Any]]:
        """Parse a Pydantic model's JSON Schema dict into a field map.

        Accepts the dict returned directly by ``Model.model_json_schema()``.

        System columns (``scouter_created_at``, ``scouter_partition_date``,
        ``scouter_batch_id``) are included automatically.

        Args:
            schema (Dict[str, Any]):
                Dict returned by ``Model.model_json_schema()``.

        Returns:
            Dict[str, Dict[str, Any]]: Mapping of field name to Arrow type descriptor
            with ``arrow_type`` (str) and ``nullable`` (bool) keys.
        """

    @staticmethod
    def compute_fingerprint(schema: Dict[str, Any]) -> str:
        """Compute a stable 16-character SHA-256 fingerprint from a JSON Schema dict.

        The fingerprint is deterministic — the same schema always yields the same value.
        Any field addition, removal, or type change yields a different value.

        Args:
            schema (Dict[str, Any]):
                Dict returned by ``Model.model_json_schema()``.

        Returns:
            str: 16-character hexadecimal fingerprint string.
        """
