from pathlib import Path
from typing import Optional

class ScouterTestServer:
    def __init__(
        self,
        cleanup: bool = True,
        rabbit_mq: bool = False,
        kafka: bool = False,
        base_path: Optional[Path] = None,
    ) -> None:
        """Instantiates the test server.

        When the test server is used as a context manager, it will start the server
        in a background thread and set the appropriate env vars so that the client
        can connect to the server. The server will be stopped when the context manager
        exits and the env vars will be reset.

        Args:
            cleanup (bool, optional):
                Whether to cleanup the server after the test. Defaults to True.
            rabbit_mq (bool, optional):
                Whether to use RabbitMQ as the transport. Defaults to False.
            kafka (bool, optional):
                Whether to use Kafka as the transport. Defaults to False.
            base_path (Optional[Path], optional):
                The base path for the server. Defaults to None. This is primarily
                used for testing loading attributes from a pyproject.toml file.
        """

    def start_server(self) -> None:
        """Starts the test server."""

    def stop_server(self) -> None:
        """Stops the test server."""

    def __enter__(self) -> "ScouterTestServer":
        """Starts the test server."""

    def __exit__(self, exc_type, exc_value, traceback) -> None:
        """Stops the test server."""

    def set_env_vars_for_client(self) -> None:
        """Sets the env vars for the client to connect to the server."""

    def remove_env_vars_for_client(self) -> None:
        """Removes the env vars for the client to connect to the server."""

    @staticmethod
    def cleanup() -> None:
        """Cleans up the test server."""

class MockConfig:
    def __init__(self) -> None:
        """Mock configuration for the ScouterQueue"""
