import functools
from typing import Any, Awaitable, Callable, TypeVar, Union

from pydantic import BaseModel
from scouter import DriftProfile, MonitorQueue
from scouter.integrations.http import HTTPConfig
from scouter.integrations.kafka import KafkaConfig

try:
    from fastapi import APIRouter, BackgroundTasks
    from fastapi import FastAPI as _FastAPI
    from fastapi import Request
    from fastapi.responses import JSONResponse
except ImportError as exc:
    raise ImportError(
        """FastAPI is not installed as a scouter extra. 
        Install scouter with the fastapi extra (scouter[fastapi]) to use the FastAPI integration."""
    ) from exc

T = TypeVar("T", bound=Union[APIRouter, _FastAPI])


class ScouterMixin:
    def __init__(self, drift_profile: DriftProfile, config: Union[KafkaConfig, HTTPConfig]) -> None:
        """Initializes the ScouterMixin to monitor model drift

        Args:
            drift_profile:
                Monitoring profile containing feature drift profiles.
            config:
                Configuration for the monitoring producer. The configured producer
                will be used to publish drift records to the monitoring server.

        Additional Args:
            *args:
                Additional arguments to pass to the parent class.
            **kwargs:
                Additional keyword arguments to pass to the parent class.

        """
        self._queue = MonitorQueue(drift_profile, config)

    def add_api_route(self, path: str, endpoint: Callable[..., Awaitable[Any]], **kwargs: Any) -> None:
        if "request" not in endpoint.__code__.co_varnames:
            raise ValueError("Request object must be passed to the endpoint function")

        assert issubclass(
            kwargs["response_model"], BaseModel
        ), "Response model must be specified as a Pydantic BaseModel"

        @functools.wraps(endpoint)
        async def wrapper(request: Request, *args: Any, **kwargs: Any) -> Any:
            # Call the original endpoint function and capture necessary values
            response_data = await endpoint(request, *args, **kwargs)

            response = JSONResponse(content=response_data.model_dump())
            background_tasks = BackgroundTasks()
            background_tasks.add_task(self._queue.insert, request.state.scouter_data)
            response.background = background_tasks

            return response

        super().add_api_route(path, wrapper, **kwargs)  # type: ignore


class ScouterRouter(ScouterMixin, APIRouter):
    def __init__(
        self,
        drift_profile: DriftProfile,
        config: Union[KafkaConfig, HTTPConfig],
        *args: Any,
        **kwargs: Any,
    ) -> None:
        APIRouter.__init__(self, *args, **kwargs)
        ScouterMixin.__init__(self, drift_profile, config)


class FastAPI(ScouterMixin, _FastAPI):
    def __init__(
        self,
        drift_profile: DriftProfile,
        config: Union[KafkaConfig, HTTPConfig],
        *args: Any,
        **kwargs: Any,
    ) -> None:
        _FastAPI.__init__(self, *args, **kwargs)
        ScouterMixin.__init__(self, drift_profile, config)
