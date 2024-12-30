import functools
import time
from contextlib import asynccontextmanager
from typing import Any, AsyncGenerator, Awaitable, Callable, Union

from pydantic import BaseModel
from scouter import MonitorQueue, PsiDriftProfile, ScouterObserver, SpcDriftProfile
from scouter.integrations.http import HTTPConfig
from scouter.integrations.kafka import KafkaConfig
from scouter.utils.logger import ScouterLogger

logger = ScouterLogger.get_logger()

try:
    from fastapi import APIRouter, BackgroundTasks, FastAPI, Request, Response
    from fastapi.responses import JSONResponse
    from starlette.middleware.base import BaseHTTPMiddleware
except ImportError as exc:
    raise ImportError(
        """FastAPI is not installed as a scouter extra. 
        Install scouter with the fastapi extra (scouter[fastapi]) to use the FastAPI integration."""
    ) from exc


class ScouterMixin:
    def __init__(
        self,
        drift_profile: Union[SpcDriftProfile, PsiDriftProfile],
        config: Union[KafkaConfig, HTTPConfig],
    ) -> None:
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
        drift_profile: Union[SpcDriftProfile, PsiDriftProfile],
        config: Union[KafkaConfig, HTTPConfig],
        *args: Any,
        **kwargs: Any,
    ) -> None:
        ScouterMixin.__init__(self, drift_profile, config)

        @asynccontextmanager
        async def lifespan(app: FastAPI) -> AsyncGenerator[None, None]:
            """Lifespan event for scouter monitoring queue.

            Args:
                app:
                    FastAPI application instance.

            Yields:
                None
            """
            logger.info("Starting scouter queue")
            yield
            logger.info("Flushing scouter queue")
            self._queue.flush()

        kwargs["lifespan"] = lifespan
        APIRouter.__init__(self, *args, **kwargs)


class _ScouterMiddleware:
    def __init__(self, observer: ScouterObserver):
        self.observer = observer

    async def __call__(self, request: Request, call_next: Callable):
        start_time = time.time()
        response: Response = await call_next(request)
        process_time = time.time() - start_time

        self.observer.add_request_metrics(
            route=request.url.path,
            latency=process_time,
            status_code=response.status_code,
        )

        return response


class FastAPIScouterObserver:
    def __init__(
        self,
        drift_profile: Union[SpcDriftProfile],
        config: Union[KafkaConfig, HTTPConfig],
    ) -> None:
        self._observer = ScouterObserver(
            repository=drift_profile.config.repository,
            name=drift_profile.config.name,
            version=drift_profile.config.version,
            config=config,
        )

    def observe(self, app: FastAPI) -> None:
        app.add_middleware(
            BaseHTTPMiddleware,
            dispatch=_ScouterMiddleware(self._observer),
        )

        @asynccontextmanager
        async def lifespan(app: FastAPI) -> AsyncGenerator[None, None]:
            # Code to run when the application starts
            logger.info("Starting scouter observer.")
            yield
            # Code to run when the application shuts down
            logger.info("Shutting down scouter observer.")
            self._observer.stop()

        if app.router.lifespan_context:
            original_lifespan = app.router.lifespan_context

            @asynccontextmanager
            async def combined_lifespan(
                app: FastAPI,
            ) -> AsyncGenerator[None, None]:
                async with original_lifespan(app):
                    async with lifespan(app):
                        yield

            app.router.lifespan_context = combined_lifespan
        else:
            app.router.lifespan_context = lifespan
