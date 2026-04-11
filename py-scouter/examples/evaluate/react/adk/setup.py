"""
Scouter queue, tracer, and instrumentor setup.

Import these into the other modules. This setup is identical to what
you would have in your production recipe service — no special eval config needed.
EvalOrchestrator switches the queue to local capture mode automatically.
"""

import os
from pathlib import Path
from typing import Union

from pydantic import BaseModel, ConfigDict
from scouter.agent import Prompt
from scouter.drift import AgentEvalProfile
from scouter.evaluate import TasksFile
from scouter.queue import ScouterQueue
from scouter.tracing import ScouterInstrumentor
from scouter.transport import GrpcConfig, MockConfig

_CWD = Path(__file__).parent


# Install ScouterInstrumentor BEFORE any ADK code runs.
# ADK calls get_tracer_provider() when building agents — Scouter must be
# registered as the global OTel provider before that happens.
scouter_instrumentor = ScouterInstrumentor()


class Config(BaseModel):
    queue: ScouterQueue
    recipe_prompt: Prompt
    customer_prompt: Prompt
    instrumentor: ScouterInstrumentor

    model_config = ConfigDict(arbitrary_types_allowed=True)


def _get_transport_config() -> Union[GrpcConfig, MockConfig]:
    app_env = os.getenv("APP_ENV", "local")
    if app_env in ("staging", "production"):
        return GrpcConfig()
    return MockConfig()


def setup() -> Config:
    recipe_prompt = Prompt.from_path(_CWD / "config" / "recipe_prompt.yaml")
    customer_prompt = Prompt.from_path(_CWD / "config" / "customer_prompt.yaml")
    tasks = TasksFile.from_path(_CWD / "config" / "receipe_tasks.yaml")

    profile = AgentEvalProfile(
        alias="recipe_agent",
        tasks=tasks,
    )

    queue = ScouterQueue.from_profile(
        profile=[profile],
        transport_config=_get_transport_config(),
    )

    scouter_instrumentor.instrument(scouter_queue=queue)

    return Config(
        queue=queue,
        recipe_prompt=recipe_prompt,
        customer_prompt=customer_prompt,
        instrumentor=scouter_instrumentor,
    )


config = setup()
