from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import yaml
from scouter.client import ScouterClient
from scouter.drift import AgentEvalConfig, AgentEvalProfile
from scouter.evaluate import TasksFile
from scouter.queue import ScouterQueue
from scouter.tracing import (
    BatchConfig,
    ScouterInstrumentor,
    flush_tracer,
    shutdown_tracer,
)
from scouter.transport import GrpcConfig

PROMPT_DIR = Path(__file__).parent / "prompts"


@dataclass(frozen=True)
class PromptSpec:
    name: str
    instruction: str
    eval_profile: AgentEvalProfile
    task_ids: set[str]


@dataclass(frozen=True)
class AttachedEval:
    agent_name: str
    profile_uid: str
    record_id: str
    session_id: str
    trace_id: str
    span_id: str


@dataclass
class ScouterUserJourney:
    client: ScouterClient
    queue: ScouterQueue
    instrumentor: ScouterInstrumentor
    router: PromptSpec
    resolution: PromptSpec
    attached_evals: list[AttachedEval] = field(default_factory=list)

    def profile_for_agent(self, agent_name: str) -> AgentEvalProfile:
        if agent_name == "router_agent":
            return self.router.eval_profile
        if agent_name == "resolution_agent":
            return self.resolution.eval_profile
        raise AssertionError(f"Unexpected agent: {agent_name}")

    def task_ids_for_profile(self, profile: AgentEvalProfile) -> set[str]:
        if profile.config.uid == self.router.eval_profile.config.uid:
            return self.router.task_ids
        if profile.config.uid == self.resolution.eval_profile.config.uid:
            return self.resolution.task_ids
        raise AssertionError(f"Unexpected profile uid: {profile.config.uid}")

    def record_attached_eval(self, attached_eval: AttachedEval) -> None:
        self.attached_evals.append(attached_eval)

    def shutdown(self) -> None:
        flush_tracer()
        self.queue.shutdown()
        self.instrumentor.uninstrument()
        shutdown_tracer()


def load_prompt_spec(prompt_name: str, run_id: str) -> PromptSpec:
    prompt_path = PROMPT_DIR / f"{prompt_name}.yaml"
    prompt_data = _load_yaml(prompt_path)
    tasks = TasksFile.from_path(prompt_path)

    config = AgentEvalConfig(
        space="scouter",
        name=f"scouter_{prompt_name}_{run_id}",
        version="0.1.0",
        sample_ratio=1.0,
    )
    profile = AgentEvalProfile(
        tasks=tasks,
        config=config,
        alias=f"{prompt_name}_evaluation",
    )

    return PromptSpec(
        name=str(prompt_data["name"]),
        instruction=str(prompt_data["instruction"]),
        eval_profile=profile,
        task_ids={task.id for task in tasks},
    )


def create_scouter_user_journey(
    *,
    client: ScouterClient,
    run_id: str,
    profile_dir: Path,
) -> ScouterUserJourney:
    router = load_prompt_spec("router", run_id)
    resolution = load_prompt_spec("resolution", run_id)

    for profile in (router.eval_profile, resolution.eval_profile):
        assert client.register_profile(profile, set_active=True)

    queue = _create_queue(
        profile_dir=profile_dir,
        router=router.eval_profile,
        resolution=resolution.eval_profile,
    )
    instrumentor = ScouterInstrumentor()
    instrumentor.instrument(
        service_name=f"scouter-user-journey-{run_id}",
        batch_config=BatchConfig(scheduled_delay_ms=200),
        scouter_queue=queue,
        propagate_baggage=False,
    )

    return ScouterUserJourney(
        client=client,
        queue=queue,
        instrumentor=instrumentor,
        router=router,
        resolution=resolution,
    )


def _create_queue(
    *,
    profile_dir: Path,
    router: AgentEvalProfile,
    resolution: AgentEvalProfile,
) -> ScouterQueue:
    profile_dir.mkdir(parents=True, exist_ok=True)
    router_path = router.save_to_json(profile_dir / "router_profile")
    resolution_path = resolution.save_to_json(profile_dir / "resolution_profile")
    return ScouterQueue.from_path(
        path={
            "router": router_path,
            "resolution": resolution_path,
        },
        transport_config=GrpcConfig(),
    )


def _load_yaml(path: Path) -> dict[str, Any]:
    with path.open(encoding="utf-8") as prompt_file:
        data = yaml.safe_load(prompt_file)
    if not isinstance(data, dict):
        raise AssertionError(f"Prompt file must contain a mapping: {path}")
    return data
