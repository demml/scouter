from __future__ import annotations

import os
from typing import Callable

from fastapi import FastAPI
from google.adk.agents import Agent
from google.adk.agents.callback_context import CallbackContext
from google.adk.runners import Runner
from google.adk.sessions import InMemorySessionService
from google.genai import types
from pydantic import BaseModel
from scouter import trace

from ..shared import get_shared_config, teardown

config = get_shared_config()
QUERY_STATE_KEY = "query"
ADK_RESPONSE_KEY = "adk_response"  # key under which llm_response is stored in EvalRecord context

AgentCallback = Callable[[str, str], None]


def look_up(query: str) -> str:
    """Look up relevant information to help answer a user query."""
    return f"Searched for: {query}"


class AgentRequest(BaseModel):
    query: str


class AgentResponse(BaseModel):
    response: str


def _emit_eval_record(query: str, response: str) -> None:
    tracer = trace.get_tracer("evaluate.agent.google")

    with tracer.start_as_current_span("google.callback") as span:
        context: dict = {"query": query, "response": response}
        span.attach_eval(
            profile_uid=config.eval_profile.config.uid,
            context=context,
        )


class GoogleAgentService:
    """Owns the ADK runner and the callback used by the interactive service."""

    def __init__(self, callback: AgentCallback | None = None) -> None:
        self._callback = callback or _emit_eval_record
        self._service = self._build_service()

    def _after_agent_callback(self, callback_context: CallbackContext) -> types.Content | None:
        events = callback_context.session.events
        query = str(callback_context.state.get(QUERY_STATE_KEY, ""))
        final_event = next((e for e in reversed(events) if e.is_final_response()), None)
        text = (
            "".join(p.text for p in final_event.content.parts if p.text)
            if final_event and final_event.content and final_event.content.parts
            else ""
        )
        self._callback(query, text)

    def _build_service(self) -> tuple[Runner, InMemorySessionService] | None:
        if not os.getenv("GOOGLE_API_KEY"):
            return None

        agent = Agent(
            model=config.model_name,
            name="ai_agent",
            description="Interactive assistant",
            instruction=config.prompt_message,
            tools=[look_up],
            after_agent_callback=self._after_agent_callback,
        )
        session_service = InMemorySessionService()
        runner = Runner(
            agent=agent,
            app_name="ai_agent",
            session_service=session_service,
        )
        return runner, session_service

    async def run(self, query: str) -> str:
        """Execute one ADK request without creating or destroying an event loop."""

        if self._service is None:
            return self._fallback_response(query)

        runner, session_service = self._service
        session = await session_service.create_session(
            app_name="ai_agent",
            user_id="evaluate_user",
            state={QUERY_STATE_KEY: query},
        )
        message = types.Content(role="user", parts=[types.Part(text=query)])
        response = ""

        async for event in runner.run_async(
            user_id="evaluate_user",
            session_id=session.id,
            new_message=message,
        ):
            if event.is_final_response() and event.content:
                for part in event.content.parts:  # type: ignore
                    if part.text:
                        response = part.text

        return response or self._fallback_response(query)

    @staticmethod
    def _fallback_response(query: str) -> str:
        lowered = query.lower()
        if "dinner" in lowered:
            return "Use one protein, one vegetable, and one starch. I can refine with your pantry."
        if "timeout" in lowered:
            return "Check timeout values, retry policy, and dependency latency."
        return "Fallback response because GOOGLE_API_KEY is not set."


def build_agent_service(callback: AgentCallback | None = None) -> GoogleAgentService:
    return GoogleAgentService(callback=callback)


_api_service = build_agent_service()

app = FastAPI(title="AI Google ADK Agent")


@app.post("/ask", response_model=AgentResponse)
async def ask(request: AgentRequest) -> AgentResponse:
    response = await _api_service.run(request.query)
    return AgentResponse(response=response)


def shutdown() -> None:
    teardown()


if __name__ == "__main__":
    import argparse
    import asyncio

    _parser = argparse.ArgumentParser(description="Run Google ADK agent example.")
    _parser.add_argument("query", help="Query to send to the agent.")
    _args = _parser.parse_args()
    _service = build_agent_service()
    print(asyncio.run(_service.run(_args.query)))
    teardown()
