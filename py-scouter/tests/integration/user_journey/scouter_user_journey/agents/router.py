from __future__ import annotations

from typing import Any

from google.adk.agents import LlmAgent
from google.adk.agents.callback_context import CallbackContext
from google.genai import types
from scouter.tracing import get_tracer

from ..setup import AttachedEval, ScouterUserJourney
from .callbacks import (
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

ROUTER_AGENT_NAME = "router_agent"


def classify_support_intent(query: str) -> dict[str, Any]:
    tracer = get_tracer("scouter_user_journey.tools")
    intent = "latency" if "slow" in query.lower() or "latency" in query.lower() else "quality"
    with tracer.start_as_current_span("tool.classify_support_intent") as span:
        span.set_attribute("tool.name", "classify_support_intent")
        span.set_attribute("support.intent", intent)
        return {"status": "success", "query": query, "intent": intent}


def build_router_agent(journey: ScouterUserJourney) -> LlmAgent:
    return LlmAgent(
        name=ROUTER_AGENT_NAME,
        model=DeterministicToolCallingLlm(
            tool_name="classify_support_intent",
            tool_args={"query": "Help investigate a slow model response."},
            final_text="The request is classified as latency support.",
        ),
        description="Classifies the request into the support intent.",
        instruction=journey.router.instruction,
        tools=[classify_support_intent],
        after_tool_callback=capture_tool_call,
        after_agent_callback=lambda callback_context: _after_router_agent(callback_context, journey),
    )


def _after_router_agent(
    callback_context: CallbackContext,
    journey: ScouterUserJourney,
) -> types.Content | None:
    state = callback_context.state
    results = tool_results(callback_context)
    classify_result = results.get("classify_support_intent", {})
    intent = str(classify_result.get("intent", ""))
    state[INTENT_STATE_KEY] = intent

    names = tool_call_names(callback_context)
    context = {
        "query": str(state.get(QUERY_STATE_KEY, "")),
        "response": "The request is classified as latency support.",
        "tool_call_names": names,
        "tool_call_path": " -> ".join(names),
        "tool_calls": list(state.get(TOOL_CALLS_STATE_KEY, [])),
        "tool_results": results,
        "intent": intent,
    }

    tracer = get_tracer("scouter_user_journey.router")
    with tracer.start_as_current_span("router.callback") as span:
        profile_uid = journey.router.eval_profile.config.uid
        attached_record_id = record_id(callback_context, ROUTER_AGENT_NAME)
        attached_session_id = session_id(callback_context)
        span.attach_eval(
            profile_uid=profile_uid,
            context=context,
            record_id=attached_record_id,
            session_id=attached_session_id,
            tags=["source=google_adk", "agent=router_agent"],
        )
        journey.record_attached_eval(
            AttachedEval(
                agent_name=ROUTER_AGENT_NAME,
                profile_uid=profile_uid,
                record_id=attached_record_id,
                session_id=attached_session_id,
                trace_id=span.trace_id,
                span_id=span.span_id,
            )
        )

    return None
