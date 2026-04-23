"""Google ADK API example for interactive evaluation.

This mirrors the non-interactive API example, but the prompts and scenarios are
interactive. The same service object owns:

1. the ADK runner/session service
2. the callback that emits `EvalRecord`s
3. the session state that lets the callback recover the original user message
"""

from __future__ import annotations

import os
from typing import Callable, Optional, cast

from fastapi import FastAPI
from google.adk.agents import Agent
from google.adk.agents.callback_context import CallbackContext
from google.adk.models.llm_response import LlmResponse
from google.adk.runners import Runner
from google.adk.sessions import InMemorySessionService
from google.genai import types
from opentelemetry import trace
from pydantic import BaseModel
from scouter.evaluate import EvalRecord
from scouter.tracing import ScouterTracer

from ..shared import get_shared_config, teardown_shared_config

config = get_shared_config()
QUERY_STATE_KEY = "query"

AgentCallback = Callable[[str, str], None]


class AgentRequest(BaseModel):
    """HTTP request model for the interactive API example."""

    query: str


class AgentResponse(BaseModel):
    """HTTP response model for the interactive API example."""

    response: str


def _emit_eval_record(query: str, response: str) -> None:
    """Emit the record users would normally send from a production callback."""
    tracer = cast(ScouterTracer, trace.get_tracer("evaluate.interactive.google"))
    with tracer.start_as_current_span("google.callback") as span:
        span.add_queue_item(
            "interactive_support_agent",
            EvalRecord(
                id=f"google_interactive_{abs(hash((query, response))) % 1_000_000}",
                context={"query": query, "response": response},
            ),
        )


class GoogleAgentService:
    """Own the ADK runner and the callback used by the interactive service."""

    def __init__(self, callback: AgentCallback | None = None) -> None:
        self._callback = callback or _emit_eval_record
        self._service = self._build_service()

    def _after_model_callback(
        self,
        callback_context: CallbackContext,
        llm_response: LlmResponse,
    ) -> Optional[LlmResponse]:
        """Emit an eval record after the model returns its final text."""
        if llm_response.partial:
            return None
        if not llm_response.content or not llm_response.content.parts:
            return None

        text = next(
            (part.text for part in llm_response.content.parts if part.text),
            None,
        )
        if text:
            query = str(callback_context.state.get(QUERY_STATE_KEY, ""))
            self._callback(query, text)
        return None

    def _build_service(self) -> tuple[Runner, InMemorySessionService] | None:
        """Create the ADK runner once so API and eval reuse the same setup."""
        if not os.getenv("GOOGLE_API_KEY"):
            return None

        agent = Agent(
            model=config.prompt.model,
            name="google_interactive_agent",
            description="Interactive assistant",
            instruction=config.prompt.message.text,
            after_model_callback=self._after_model_callback,
        )
        session_service = InMemorySessionService()
        runner = Runner(
            agent=agent,
            app_name="scouter_google_interactive",
            session_service=session_service,
        )
        return runner, session_service

    async def run(self, query: str) -> str:
        """Execute one ADK request without creating or destroying an event loop."""

        if self._service is None:
            response = self._fallback_response(query)
            self._callback(query, response)
            return response

        runner, session_service = self._service
        session = await session_service.create_session(
            app_name="scouter_google_interactive",
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
                parts = event.content.parts
                if not isinstance(parts, list):
                    continue
                for part in parts:
                    if part.text:
                        response = part.text
                        break
                if response:
                    break

        if not response:
            response = self._fallback_response(query)
            self._callback(query, response)
        return response

    @staticmethod
    def _fallback_response(query: str) -> str:
        """Return deterministic local answers when credentials are not configured."""
        lowered = query.lower()
        if "dinner" in lowered:
            return "Use one protein, one vegetable, and one starch. I can refine with your pantry."
        if "timeout" in lowered:
            return "Check timeout values, retry policy, and dependency latency."
        return "Fallback response because GOOGLE_API_KEY is not set."


def build_agent_service(callback: AgentCallback | None = None) -> GoogleAgentService:
    """Build the service object used by both the API and the eval example."""
    return GoogleAgentService(callback=callback)


_api_service = build_agent_service()

app = FastAPI(title="Scouter Google Interactive Agent")


@app.post("/ask", response_model=AgentResponse)
async def ask(request: AgentRequest) -> AgentResponse:
    """Serve the ADK agent through FastAPI using the server's existing loop."""
    response = await _api_service.run(request.query)
    return AgentResponse(response=response)


def shutdown() -> None:
    """Tear down shared Scouter instrumentation for the example process."""
    teardown_shared_config()
