from __future__ import annotations

from typing import Any

from google.adk.agents.callback_context import CallbackContext
from google.adk.tools.base_tool import BaseTool

QUERY_STATE_KEY = "query"
TOOL_CALLS_STATE_KEY = "tool_calls"
INTENT_STATE_KEY = "intent"
ANSWER_STATE_KEY = "answer"


def capture_tool_call(
    tool: BaseTool,
    args: dict[str, Any],
    tool_context: CallbackContext,
    tool_response: dict[str, Any],
) -> None:
    tool_calls = list(tool_context.state.get(TOOL_CALLS_STATE_KEY, []))
    tool_calls.append(
        {
            "name": tool.name,
            "arguments": args,
            "response": tool_response,
        }
    )
    tool_context.state[TOOL_CALLS_STATE_KEY] = tool_calls


def tool_call_names(callback_context: CallbackContext) -> list[str]:
    tool_calls = list(callback_context.state.get(TOOL_CALLS_STATE_KEY, []))
    return [str(call.get("name", "")) for call in tool_calls]


def tool_results(callback_context: CallbackContext) -> dict[str, Any]:
    results: dict[str, Any] = {}
    for call in callback_context.state.get(TOOL_CALLS_STATE_KEY, []):
        if isinstance(call, dict) and isinstance(call.get("response"), dict):
            results[str(call["name"])] = call["response"]
    return results


def session_id(callback_context: CallbackContext) -> str:
    return str(callback_context.session.id)


def record_id(callback_context: CallbackContext, agent_name: str) -> str:
    return f"{agent_name}:{callback_context.invocation_id}"
