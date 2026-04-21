"""
CrewAI crew factory for the multi-agent eval example.

Two sub-agents run sequentially:
  researcher  — investigates the query, emits EvalRecord under alias "researcher"
  analyst     — receives research and distills insights, emits under alias "analyst"

LLM config (model, provider, system instruction) comes from scouter Prompt objects
loaded in setup.py. provider.as_str() maps to the LiteLLM prefix used by CrewAI.
"""

from typing import cast
import os
from crewai import Agent, Crew, LLM, Process, Task
from crewai.tasks.task_output import TaskOutput
from opentelemetry import trace
from scouter.agent import Prompt
from scouter.evaluate import EvalRecord
from scouter.tracing import BaseTracer

_GOOGLE_API_KEY = os.getenv("GOOGLE_API_KEY")

gemini_llm = LLM(
    model="gemini/gemini-flash-latest",
    api_key=_GOOGLE_API_KEY,
    temperature=1.0,
)


def _emit(alias: str, text: str) -> None:
    tracer = cast(BaseTracer, trace.get_tracer(alias))
    with tracer.start_as_current_span(f"{alias}.eval") as span:
        span.add_queue_item(alias, EvalRecord(context={"response": text}))


def build_crew(query: str, researcher_prompt: Prompt, analyst_prompt: Prompt) -> Crew:
    researcher_llm = gemini_llm
    analyst_llm = gemini_llm

    researcher = Agent(
        role="Research Specialist",
        goal="Conduct thorough research and produce comprehensive, factual summaries",
        backstory=researcher_prompt.system_instructions[0].text,
        llm=researcher_llm,
        verbose=False,
    )

    analyst = Agent(
        role="Strategic Analyst",
        goal="Transform research findings into concise strategic insights and recommendations",
        backstory=analyst_prompt.system_instructions[0].text,
        llm=analyst_llm,
        verbose=False,
    )

    def on_research_done(output: TaskOutput) -> None:
        _emit("researcher", output.raw)

    def on_analysis_done(output: TaskOutput) -> None:
        _emit("analyst", output.raw)

    research_task = Task(
        description=(
            f"Research the following topic thoroughly: {query}\n\n"
            "Produce a structured summary covering: key facts, major players or "
            "developments, and open questions. Aim for 3-5 concise paragraphs."
        ),
        expected_output="A factual research summary (3-5 paragraphs)",
        agent=researcher,
        callback=on_research_done,
    )

    analysis_task = Task(
        description=(
            "Using the research provided in context, extract the top 3 strategic insights "
            "and for each one provide a concrete recommendation. "
            "Structure your response as: Insight → Recommendation."
        ),
        expected_output="3 strategic insights each paired with a concrete recommendation",
        agent=analyst,
        callback=on_analysis_done,
        context=[research_task],
    )

    return Crew(
        agents=[researcher, analyst],
        tasks=[research_task, analysis_task],
        process=Process.sequential,
        verbose=False,
    )
