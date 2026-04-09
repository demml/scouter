"""
ADK recipe agent with after_model_callback.

The callback fires after every model response and emits an EvalRecord into
the queue. This captures every reply across all reactive turns so the profile
tasks have data to evaluate. Returning None leaves the response unchanged.
"""

from typing import Optional, cast

from google.adk.agents import Agent
from google.adk.agents.callback_context import CallbackContext
from google.adk.models.llm_response import LlmResponse
from opentelemetry import trace
from scouter.evaluate import EvalRecord
from scouter.tracing import BaseTracer

from ..setup import config


def after_model_callback(callback_context: CallbackContext, llm_response: LlmResponse) -> Optional[LlmResponse]:
    if llm_response.partial:
        return None
    if not llm_response.content or not llm_response.content.parts:
        return None

    text = next(
        (part.text for part in llm_response.content.parts if part.text),
        None,
    )
    if text is None:
        return None

    tracer = cast(BaseTracer, trace.get_tracer("recipe_agent"))
    with tracer.start_as_current_span("recipe_agent.eval") as span:
        span.add_queue_item(
            "recipe_agent",
            EvalRecord(
                context={"response": text},
                id=f"recipe_{abs(hash(text)) % 10_000}",
            ),
        )

    return None


recipe_agent = Agent(
    model=config.recipe_prompt.model,
    name="recipe_agent",
    description="Helps users find recipes and cooking advice.",
    instruction=config.recipe_prompt.message.text,
    after_model_callback=after_model_callback,
)
