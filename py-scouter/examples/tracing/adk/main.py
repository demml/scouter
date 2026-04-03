import asyncio

from google.adk.agents import Agent
from google.adk.runners import Runner
from google.adk.sessions import InMemorySessionService
from google.genai import types
from scouter.tracing import ScouterInstrumentor
from scouter.transport import GrpcConfig

# Sets Scouter as the global OTel TracerProvider before any ADK code runs.
# ADK calls get_tracer_provider() internally — it picks this up automatically.
ScouterInstrumentor().instrument(
    transport_config=GrpcConfig(),
    attributes={"service.name": "adk-hello-agent"},
)


def get_current_time(city: str) -> dict:
    """Returns the current time in a specified city."""
    return {"city": city, "time": "10:30 AM"}


root_agent = Agent(
    model="gemini-3-flash-preview",
    name="hello_agent",
    description="Tells the current time in a specified city.",
    instruction="You are a helpful assistant. Use get_current_time to answer questions about the current time.",
    tools=[get_current_time],
)


async def main() -> None:
    session_service = InMemorySessionService()
    runner = Runner(
        agent=root_agent,
        app_name="hello_app",
        session_service=session_service,
    )

    session = await session_service.create_session(
        app_name="hello_app",
        user_id="user_1",
    )

    message = types.Content(
        role="user",
        parts=[types.Part(text="What time is it in New York?")],
    )

    async for event in runner.run_async(
        user_id="user_1",
        session_id=session.id,
        new_message=message,
    ):
        if event.is_final_response():
            print(event.content.parts[0].text)  # type: ignore

    ScouterInstrumentor().uninstrument()


if __name__ == "__main__":
    asyncio.run(main())
