import functools
from typing import Any, Awaitable, Callable, Union

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


class ScouterRouter(APIRouter):
    def __init__(
        self,
        drift_profile: DriftProfile,
        config: Union[KafkaConfig, HTTPConfig],
        *args,
        **kwargs,
    ) -> None:
        """Initializes the ScouterRouter to monitor model drift

        Args:
            drift_profile:
                Monitoring profile containing feature drift profiles.
            config:
                Configuration for the monitoring producer. The configured producer
                will be used to publish drift records to the monitoring server.

        Additional Args:
            *args:
                Additional arguments to pass to the FastAPI router.
            **kwargs:
                Additional keyword arguments to pass to the FastAPI router.

        """

        super().__init__(*args, **kwargs)
        self._queue = MonitorQueue(drift_profile, config)

    def add_api_route(self, path: str, endpoint: Callable[..., Awaitable[Any]], **kwargs: Any) -> None:
        if "request" not in endpoint.__code__.co_varnames:
            raise ValueError("Endpoint must have a request parameter if using Scouter integration")

        assert issubclass(
            kwargs["response_model"], BaseModel
        ), "Response model must be a sepcified as a Pydantic BaseModel"

        @functools.wraps(endpoint)
        async def wrapper(request: Request, *args: Any, **kwargs: Any) -> Any:
            # Call the original endpoint function and capture necessary values
            response_data: BaseModel = await endpoint(request, *args, **kwargs)

            response = JSONResponse(content=response_data.model_dump())

            background_tasks = BackgroundTasks()
            background_tasks.add_task(self._queue.insert, request.state.scouter_data)
            response.background = background_tasks

            return response

        super().add_api_route(path, wrapper, **kwargs)


class FastAPI(_FastAPI):
    def __init__(
        self,
        drift_profile: DriftProfile,
        config: Union[KafkaConfig, HTTPConfig],
        *args: Any,
        **kwargs: Any,
    ) -> None:
        """Initializes the FastAPI application with Scouter monitoring.

        Args:
            drift_profile:
                Monitoring profile containing feature drift profiles.
            config:
                Configuration for the monitoring producer. The configured producer
                will be used to publish drift records to the monitoring server.

        Additional Args:
            *args:
                Additional arguments to pass to the Fast

            **kwargs:
                Additional keyword arguments to pass to the Fast
        """
        super().__init__(*args, **kwargs)
        self._queue = MonitorQueue(drift_profile, config)

    def add_api_route(self, path: str, endpoint: Callable[..., Awaitable[Any]], **kwargs: Any) -> None:
        print(endpoint.__code__.co_varnames)
        # check if the endpoing has a request parameter
        if "request" not in endpoint.__code__.co_varnames:
            raise ValueError("Endpoint must have a request parameter")

        @functools.wraps(endpoint)
        async def wrapper(request: Request, *args: Any, **kwargs: Any) -> Any:
            # Call the original endpoint function and capture necessary values
            response_data = await endpoint(request, *args, **kwargs)
            background_tasks = BackgroundTasks()
            background_tasks.add_task(self._queue.insert, request.state.scouter_data)

            # Create a JSONResponse and attach the background tasks
            response = JSONResponse(content=response_data.dict())
            response.background = background_tasks

            return response

        super().add_api_route(path, wrapper, **kwargs)
