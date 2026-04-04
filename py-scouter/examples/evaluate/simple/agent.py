"""
ADK agent definition with after_model_callback.

The callback fires after every model response. It emits an EvalRecord into
the queue so that the profile tasks in setup.py have data to evaluate.
Returning None from the callback leaves the response unchanged.
"""

from typing import Optional, cast

from google.adk.agents import Agent
from google.adk.agents.callback_context import CallbackContext
from google.adk.models.llm_response import LlmResponse
from opentelemetry import trace
from scouter.evaluate import EvalRecord
from scouter.tracing import BaseTracer

from .setup import config


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

    tracer = cast(BaseTracer, trace.get_tracer("qa_agent"))
    with tracer.start_as_current_span("qa_agent.eval") as span:
        span.add_queue_item(
            "qa_agent",
            EvalRecord(
                context={"response": text},
                id=f"qa_{abs(hash(text)) % 10_000}",
            ),
        )

    return None


qa_agent = Agent(
    model=config.prompt.model,
    name="qa_agent",
    description="Answers factual questions concisely.",
    instruction=config.prompt.message.text,
    after_model_callback=after_model_callback,
)
