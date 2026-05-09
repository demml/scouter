from __future__ import annotations

from google.adk.agents import SequentialAgent
from google.adk.runners import Runner
from google.adk.sessions import InMemorySessionService
from google.genai import types
from scouter.tracing import get_tracer

from ..setup import ScouterUserJourney
from .callbacks import QUERY_STATE_KEY, TOOL_CALLS_STATE_KEY
from .resolution import build_resolution_agent
from .router import build_router_agent

APP_NAME = "scouter_user_journey"
ROOT_AGENT_NAME = "scouter_multi_agent_pipeline"


def build_runner(journey: ScouterUserJourney) -> tuple[Runner, InMemorySessionService]:
    root_agent = SequentialAgent(
        name=ROOT_AGENT_NAME,
        sub_agents=[
            build_router_agent(journey),
            build_resolution_agent(journey),
        ],
    )
    session_service = InMemorySessionService()
    runner = Runner(
        agent=root_agent,
        app_name=APP_NAME,
        session_service=session_service,
    )
    return runner, session_service


async def run_agent_turn(
    runner: Runner,
    session_service: InMemorySessionService,
    *,
    query: str,
) -> str:
    session = await session_service.create_session(
        app_name=APP_NAME,
        user_id="evaluate_user",
        state={
            QUERY_STATE_KEY: query,
            TOOL_CALLS_STATE_KEY: [],
        },
    )
    message = types.Content(role="user", parts=[types.Part(text=query)])
    response = ""

    tracer = get_tracer("scouter_user_journey.request")
    with tracer.start_as_current_span("agent.request") as span:
        span.set_attribute("adk.session_id", session.id)
        span.set_attribute("agent.query", query)
        async for event in runner.run_async(
            user_id="evaluate_user",
            session_id=session.id,
            new_message=message,
        ):
            if event.is_final_response() and event.content and event.content.parts:
                response = "".join(part.text or "" for part in event.content.parts)

    return response
