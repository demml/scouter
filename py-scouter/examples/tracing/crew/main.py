from crewai import Agent, Crew, Task  # type: ignore
from crewai import LLM  # type: ignore
import os
from scouter.tracing import ScouterInstrumentor
from scouter.transport import GrpcConfig

gemini_api_key = os.getenv("GOOGLE_API_KEY")

gemini_llm = LLM(
    model="gemini/gemini-3-flash-preview",
    api_key=gemini_api_key,
    temperature=1.0,  # Use the Gemini 3 recommended temperature
)

ScouterInstrumentor().instrument(
    transport_config=GrpcConfig(),
    attributes={"service.name": "crewai-service"},
)

# Agents and tasks defined after instrument() — spans route through Scouter
researcher = Agent(
    role="Researcher",
    goal="Summarize what OpenTelemetry is in one paragraph",
    backstory="Expert at reading technical documentation",
    allow_delegation=False,
    verbose=True,
    llm=gemini_llm,
)

task = Task(
    description="Write a one-paragraph summary of what OpenTelemetry is.",
    expected_output="A concise paragraph suitable for a developer README.",
    agent=researcher,
)

crew = Crew(
    agents=[researcher],
    tasks=[task],
    verbose=True,
    tracing=True,
)
result = crew.kickoff()
print(result)
