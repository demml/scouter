from __future__ import annotations

import asyncio
import os
from typing import Any, Callable, cast

from fastapi import FastAPI
from opentelemetry import trace
from pydantic import BaseModel
from scouter.evaluate import EvalRecord
from scouter.tracing import BaseTracer

from ..shared import get_shared_config, teardown_shared_config

config = get_shared_config()

AgentCallback = Callable[[str, str], None]


class AgentRequest(BaseModel):
    query: str


class AgentResponse(BaseModel):
    response: str


def _emit_eval_record(query: str, response: str) -> None:
    tracer = cast(BaseTracer, trace.get_tracer("evaluate.non_interactive.google"))
    with tracer.start_as_current_span("google.callback") as span:
        span.add_queue_item(
            "support_agent",
            EvalRecord(
                id=f"google_{abs(hash((query, response))) % 1_000_000}",
                context={"query": query, "response": response},
            ),
        )


_active_callback: AgentCallback = _emit_eval_record


def _after_model_callback(callback_context, llm_response):  # type: ignore[no-untyped-def]
    del callback_context
    if llm_response.partial:
        return None
    if not llm_response.content or not llm_response.content.parts:
        return None

    text = next((part.text for part in llm_response.content.parts if part.text), None)
    if text:
        _active_callback("adk_query", text)
    return None


_runner: Any = None
_session_service: Any = None

if os.getenv("GOOGLE_API_KEY"):
    from google.adk.agents import Agent
    from google.adk.runners import Runner
    from google.adk.sessions import InMemorySessionService

    _agent = Agent(
        model=config.prompt.model,
        name="google_non_interactive_agent",
        description="Factual support assistant",
        instruction=config.prompt.message.text,
        after_model_callback=_after_model_callback,
    )
    _session_service = InMemorySessionService()
    _runner = Runner(
        agent=_agent,
        app_name="scouter_google_non_interactive",
        session_service=_session_service,
    )


def _fallback_response(query: str) -> str:
    lowered = query.lower()
    if "france" in lowered:
        return "Paris is the capital of France."
    if "water" in lowered:
        return "The chemical formula for water is H2O."
    if "largest planet" in lowered:
        return "Jupiter is the largest planet in our solar system."
    return "Fallback response because GOOGLE_API_KEY is not set."


async def run_agent_async(query: str, callback: AgentCallback | None = None) -> str:
    global _active_callback  # pylint: disable=global-statement
    _active_callback = callback or _emit_eval_record

    if _runner is None or _session_service is None:
        response = _fallback_response(query)
        _active_callback(query, response)
        return response
    runner = _runner
    session_service = _session_service

    from google.genai import types

    session = await session_service.create_session(  # type: ignore[attr-defined]
        app_name="scouter_google_non_interactive",
        user_id="evaluate_user",
    )
    message = types.Content(role="user", parts=[types.Part(text=query)])
    response = ""

    async for event in runner.run_async(  # type: ignore[attr-defined]
        user_id="evaluate_user",
        session_id=session.id,
        new_message=message,
    ):
        if event.is_final_response() and event.content:
            for part in event.content.parts:  # type: ignore[attr-defined]
                if part.text:
                    response = part.text
                    break
            if response:
                break

    if not response:
        response = _fallback_response(query)
        _active_callback(query, response)
    return response


def run_agent(query: str, callback: AgentCallback | None = None) -> str:
    return asyncio.run(run_agent_async(query, callback=callback))


app = FastAPI(title="Scouter Google Non-Interactive Agent")


@app.post("/ask", response_model=AgentResponse)
async def ask(request: AgentRequest) -> AgentResponse:
    return AgentResponse(response=await run_agent_async(request.query))


def shutdown() -> None:
    teardown_shared_config()
