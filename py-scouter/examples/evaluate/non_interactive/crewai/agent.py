from __future__ import annotations

import os
from typing import Any, Callable

from fastapi import FastAPI
from openinference.instrumentation.crewai import CrewAIInstrumentor
from pydantic import BaseModel
from scouter import trace
from scouter.evaluate import EvalRecord

from ..shared import get_shared_config, teardown_shared_config

config = get_shared_config()
_crewai_instrumentor = CrewAIInstrumentor()
_crewai_instrumentor.instrument(skip_dep_check=True, tracer_provider=trace.get_tracer_provider())

AgentCallback = Callable[[str, str], None]


class AgentRequest(BaseModel):
    query: str


class AgentResponse(BaseModel):
    response: str


def _emit_eval_record(query: str, response: str) -> None:
    tracer = trace.get_tracer("evaluate.non_interactive.crewai")
    with tracer.start_as_current_span("crewai.callback") as span:
        span.add_queue_item(
            "support_agent",
            EvalRecord(
                id=f"crewai_{abs(hash((query, response))) % 1_000_000}",
                context={"query": query, "response": response},
            ),
        )


def _build_crew(query: str, callback: AgentCallback):
    from crewai import LLM, Agent, Crew, Task
    from crewai.tasks.task_output import TaskOutput

    llm = LLM(
        model="gemini/gemini-2.5-flash",
        temperature=0.0,
    )
    qa_agent = Agent(
        role="Support QA Assistant",
        goal="Provide factual short answers.",
        backstory=config.prompt.message.text,
        llm=llm,
        verbose=False,
    )

    def on_task_complete(output: TaskOutput) -> None:
        callback(query, output.raw)

    task = Task(
        description=query,
        expected_output="A short factual answer.",
        agent=qa_agent,
    )
    return Crew(
        agents=[qa_agent],
        tasks=[task],
        tracing=True,
        verbose=False,
        task_callback=on_task_complete,
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


def run_agent(query: str, callback: AgentCallback | None = None) -> str:
    on_response = callback or _emit_eval_record

    if not os.getenv("GOOGLE_API_KEY"):
        response = _fallback_response(query)
        on_response(query, response)
        return response

    from crewai.crews.crew_output import CrewOutput

    result: Any = _build_crew(query=query, callback=on_response).kickoff()
    if isinstance(result, CrewOutput):
        return result.raw
    return str(result)


app = FastAPI(title="Scouter CrewAI Non-Interactive Agent")


@app.post("/ask", response_model=AgentResponse)
def ask(request: AgentRequest) -> AgentResponse:
    return AgentResponse(response=run_agent(request.query))


def shutdown() -> None:
    _crewai_instrumentor.uninstrument()
    teardown_shared_config()
