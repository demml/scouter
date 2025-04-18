import functools
from contextlib import asynccontextmanager
from typing import Any, AsyncGenerator, Awaitable, Callable, Dict, List

from pydantic import BaseModel
from scouter.queue import DriftTransportConfig, ScouterQueue

try:
    from fastapi import APIRouter, BackgroundTasks, FastAPI, Request
    from fastapi.responses import JSONResponse
except ImportError as exc:
    raise ImportError(
        """FastAPI is not installed as a scouter extra. 
        Install scouter with the fastapi extra (scouter[fastapi]) to use the FastAPI integration."""
    ) from exc


class ScouterMixin:
    def __init__(
        self,
        transport: List[DriftTransportConfig],
    ) -> None:

        self._queue: Dict[str, ScouterQueue] = {t.id: ScouterQueue(t) for t in transport}

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

            for t in self._queue:
                background_tasks.add_task(self._queue[t].insert, request.state.scouter_data[t])

            response.background = background_tasks

            return response

        super().add_api_route(path, wrapper, **kwargs)  # type: ignore


class ScouterRouter(ScouterMixin, APIRouter):
    def __init__(
        self,
        transport: List[DriftTransportConfig],
        *args: Any,
        **kwargs: Any,
    ) -> None:

        ScouterMixin.__init__(self, transport)

        @asynccontextmanager
        async def lifespan(app: FastAPI) -> AsyncGenerator[None, None]:
            """Lifespan event for scouter monitoring queue.

            Args:
                app:
                    FastAPI application instance.

            Yields:
                None
            """
            yield

            for t in self._queue:
                self._queue[t].flush()

        kwargs["lifespan"] = lifespan
        APIRouter.__init__(self, *args, **kwargs)
