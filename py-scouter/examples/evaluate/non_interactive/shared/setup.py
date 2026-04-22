from __future__ import annotations
from pydantic import BaseModel, ConfigDict

import os
from functools import lru_cache
from pathlib import Path
from typing import Union

from scouter.agent import Prompt
from scouter.drift import AgentEvalProfile
from scouter.evaluate import EvalScenarios, TasksFile
from scouter.queue import ScouterQueue
from scouter.tracing import ScouterInstrumentor
from scouter.transport import GrpcConfig, MockConfig

_BASE_DIR = Path(__file__).resolve().parent
TransportConfig = Union[GrpcConfig, MockConfig]


class SharedConfig(BaseModel):
    queue: ScouterQueue
    prompt: Prompt
    scenarios: EvalScenarios
    instrumentor: ScouterInstrumentor

    model_config = ConfigDict(arbitrary_types_allowed=True)


def _transport_config() -> TransportConfig:
    if os.getenv("APP_ENV") in {"staging", "production"}:
        return GrpcConfig()
    return MockConfig()


@lru_cache(maxsize=1)
def get_shared_config() -> SharedConfig:
    prompt = Prompt.from_path(_BASE_DIR / "prompt.yaml")
    tasks = TasksFile.from_path(_BASE_DIR / "tasks.yaml")
    scenarios = EvalScenarios.from_path(_BASE_DIR / "scenarios.jsonl")

    profile = AgentEvalProfile(alias="support_agent", tasks=tasks)
    queue = ScouterQueue.from_profile(
        profile=[profile],
        transport_config=_transport_config(),
    )

    instrumentor = ScouterInstrumentor()
    instrumentor.instrument(scouter_queue=queue)

    return SharedConfig(
        queue=queue,
        prompt=prompt,
        scenarios=scenarios,
        instrumentor=instrumentor,
    )


def teardown_shared_config() -> None:
    try:
        config = get_shared_config()
    except Exception:  # noqa: BLE001 pylint: disable=broad-except
        return

    config.instrumentor.uninstrument()
    get_shared_config.cache_clear()
