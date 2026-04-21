"""
Scouter queue and OpenTelemetry setup for the multi-agent CrewAI example.

Call order matters:
  1. ScouterInstrumentor() at module load (before CrewAI imports)
  2. scouter_instrumentor.instrument(queue=...) after queue is built
  3. CrewAIInstrumentor().instrument() last — uses the scouter OTel provider

Requires:
  pip install crewai openinference-instrumentation-crewai
  GEMINI_API_KEY in environment
"""

from chromadb.telemetry.opentelemetry import tracer

import os
from pathlib import Path

from openinference.instrumentation.crewai import CrewAIInstrumentor  # type: ignore
from pydantic import BaseModel, ConfigDict
from scouter.agent import Prompt
from scouter.drift import AgentEvalProfile
from scouter.evaluate import TasksFile
from scouter.queue import ScouterQueue
from scouter.tracing import ScouterInstrumentor
from scouter.transport import GrpcConfig, MockConfig
from opentelemetry.sdk.trace import TracerProvider

_DIR = Path(__file__).parent

# Must be instantiated before any CrewAI / OTel code that calls
# get_tracer_provider(), so it can register as the global OTel provider.
scouter_instrumentor = ScouterInstrumentor()


class Config(BaseModel):
    queue: ScouterQueue
    researcher_prompt: Prompt
    analyst_prompt: Prompt
    instrumentor: ScouterInstrumentor
    crewai_instrumentor: CrewAIInstrumentor

    model_config = ConfigDict(arbitrary_types_allowed=True)


def _transport():
    if os.getenv("APP_ENV") in ("staging", "production"):
        return GrpcConfig()
    return MockConfig()


def setup() -> Config:
    researcher_prompt = Prompt.from_path(_DIR / "researcher_prompt.yaml")
    analyst_prompt = Prompt.from_path(_DIR / "analyst_prompt.yaml")

    researcher_tasks = TasksFile.from_path(_DIR / "researcher_tasks.yaml")
    analyst_tasks = TasksFile.from_path(_DIR / "analyst_tasks.yaml")

    researcher_profile = AgentEvalProfile(alias="researcher", tasks=researcher_tasks)
    analyst_profile = AgentEvalProfile(alias="analyst", tasks=analyst_tasks)

    queue = ScouterQueue.from_profile(
        profile=[researcher_profile, analyst_profile],
        transport_config=_transport(),
    )

    scouter_instrumentor.instrument(scouter_queue=queue)
    crewai_instrumentor = CrewAIInstrumentor()
    crewai_instrumentor.instrument(
        skip_dep_check=True,
        tracer_provider=TracerProvider(),
    )

    return Config(
        queue=queue,
        researcher_prompt=researcher_prompt,
        analyst_prompt=analyst_prompt,
        instrumentor=scouter_instrumentor,
        crewai_instrumentor=crewai_instrumentor,
    )


config = setup()
