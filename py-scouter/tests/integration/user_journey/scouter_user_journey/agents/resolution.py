from __future__ import annotations

from typing import Any

from google.adk.agents import LlmAgent
from google.adk.agents.callback_context import CallbackContext
from google.genai import types
from scouter.tracing import get_tracer

from ..setup import AttachedEval, ScouterUserJourney
from .callbacks import (
    ANSWER_STATE_KEY,
    INTENT_STATE_KEY,
    QUERY_STATE_KEY,
    TOOL_CALLS_STATE_KEY,
    capture_tool_call,
    record_id,
    session_id,
    tool_call_names,
    tool_results,
)
from .llm import DeterministicToolCallingLlm

RESOLUTION_AGENT_NAME = "resolution_agent"
RESOLUTION_TEXT = "Check model latency, queue backlog, and retry budgets."


def lookup_latency_runbook(query: str) -> dict[str, Any]:
    tracer = get_tracer("scouter_user_journey.tools")
    with tracer.start_as_current_span("tool.lookup_latency_runbook") as span:
        span.set_attribute("tool.name", "lookup_latency_runbook")
        span.set_attribute("support.intent", "latency")
        return {
            "status": "success",
            "intent": "latency",
            "query": query,
            "next_step": RESOLUTION_TEXT,
        }


def collect_quality_signals(query: str) -> dict[str, Any]:
    tracer = get_tracer("scouter_user_journey.tools")
    with tracer.start_as_current_span("tool.collect_quality_signals") as span:
        span.set_attribute("tool.name", "collect_quality_signals")
        span.set_attribute("support.intent", "quality")
        return {
            "status": "success",
            "intent": "quality",
            "query": query,
            "next_step": "Review retrieval quality, prompt inputs, and response grounding.",
        }


def build_resolution_agent(journey: ScouterUserJourney) -> LlmAgent:
    return LlmAgent(
        name=RESOLUTION_AGENT_NAME,
        model=DeterministicToolCallingLlm(
            tool_name="lookup_latency_runbook",
            tool_args={"query": "Help investigate a slow model response."},
            final_text=RESOLUTION_TEXT,
        ),
        description="Calls the intent-specific tool and writes the final answer.",
        instruction=journey.resolution.instruction,
        tools=[lookup_latency_runbook, collect_quality_signals],
        after_tool_callback=capture_tool_call,
        after_agent_callback=lambda callback_context: _after_resolution_agent(callback_context, journey),
    )


def _after_resolution_agent(
    callback_context: CallbackContext,
    journey: ScouterUserJourney,
) -> types.Content | None:
    state = callback_context.state
    names = tool_call_names(callback_context)
    response = RESOLUTION_TEXT
    state[ANSWER_STATE_KEY] = response

    context = {
        "query": str(state.get(QUERY_STATE_KEY, "")),
        "response": response,
        "answer": response,
        "quality_score": 1,
        "tool_call_names": names,
        "tool_call_path": " -> ".join(names),
        "tool_calls": list(state.get(TOOL_CALLS_STATE_KEY, [])),
        "tool_results": tool_results(callback_context),
        "intent": str(state.get(INTENT_STATE_KEY, "")),
    }

    tracer = get_tracer("scouter_user_journey.resolution")
    with tracer.start_as_current_span("resolution.callback") as span:
        profile_uid = journey.resolution.eval_profile.config.uid
        attached_record_id = record_id(callback_context, RESOLUTION_AGENT_NAME)
        attached_session_id = session_id(callback_context)
        span.attach_eval(
            profile_uid=profile_uid,
            context=context,
            record_id=attached_record_id,
            session_id=attached_session_id,
            tags=["source=google_adk", "agent=resolution_agent"],
        )
        journey.record_attached_eval(
            AttachedEval(
                agent_name=RESOLUTION_AGENT_NAME,
                profile_uid=profile_uid,
                record_id=attached_record_id,
                session_id=attached_session_id,
                trace_id=span.trace_id,
                span_id=span.span_id,
            )
        )

    return None
