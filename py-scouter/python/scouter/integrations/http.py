from enum import Enum
from typing import Any, Dict, Optional, cast

import httpx
from pydantic import BaseModel
from scouter.integrations.base import BaseProducer
from scouter.utils.logger import ScouterLogger
from scouter.utils.types import ProducerTypes
from tenacity import retry, stop_after_attempt

from .._scouter import DriftServerRecord

logger = ScouterLogger.get_logger()
MESSAGE_MAX_BYTES_DEFAULT = 2097164


class RequestType(str, Enum):
    GET = "GET"
    POST = "POST"


class ApiRoutes:
    TOKEN = "auth/token"
    INSERT = "drift"


class HTTPConfig(BaseModel):
    server_url: str
    username: str
    password: str
    token: str = "empty"

    @property
    def type(self) -> str:
        return ProducerTypes.Http.value


_TIMEOUT_CONFIG = httpx.Timeout(10, read=120, write=120)


class HTTPProducer(BaseProducer):
    def __init__(self, config: HTTPConfig) -> None:
        """Initializes the HTTPProducer

        Args:
            config:
                Configuration for the

        """
        self._config = config
        self.client = httpx.Client()
        self.form_data = {
            "username": self._config.username,
            "password": self._config.password,
        }
        self._refresh_token()
        self.client.timeout = _TIMEOUT_CONFIG

    def _refresh_token(self) -> None:
        """Refreshes bearer token."""
        response = self.client.post(
            url=f"{self._config.server_url}/{ApiRoutes.TOKEN}",
            data=self.form_data,
        )
        res = response.json()

        # check if token is in response
        if "access_token" not in res:
            raise ValueError(f"Failed to get access token: {res.get('detail')}")

        self._auth_token = res["access_token"]
        self.client.headers["Authorization"] = f"Bearer {self._auth_token}"

    @retry(reraise=True, stop=stop_after_attempt(3))
    def request(self, route: str, request_type: RequestType, **kwargs: Any) -> Dict[str, Any]:
        """Makes a request to the server

        Args:
            route:
                Route to make request to
            request_type:
                Type of request to make
            **kwargs:
                Keyword arguments for request

        Returns:
            Response from server
        """
        try:
            url = f"{self._config.server_url}/{route}"
            response = getattr(self.client, request_type.value.lower())(url=url, **kwargs)

            if response.status_code == 200:
                return cast(Dict[str, Any], response.json())

            detail = response.json().get("detail")
            self._refresh_token()

            raise ValueError(f"""Failed to make server call for {request_type} request Url: {route}, {detail}""")

        except Exception as exc:
            raise exc

    def publish(self, record: DriftServerRecord) -> None:
        """Publishes drift record to a kafka topic with retries.

        If the message delivery fails, the message is retried up to `max_retries` times before raising an error.

        Args:
            record:

        Raises:
            ValueError: When max_retries is invalid.
        """
        self.request(
            route=ApiRoutes.INSERT,
            request_type=RequestType.POST,
            json=record.to_dict(),
        )

    def flush(self, timeout: Optional[float] = None) -> None:
        """Flushes the producer"""
        logger.info("Flushing not supported for HTTP producer.")

    @staticmethod
    def type() -> str:
        return ProducerTypes.Http.value
