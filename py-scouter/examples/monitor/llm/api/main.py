# pylint: disable=invalid-name
from contextlib import asynccontextmanager
from pathlib import Path

from fastapi import FastAPI, Request
from scouter.genai import Agent, Prompt
from scouter.logging import LoggingConfig, LogLevel, RustyLogger
from scouter.queue import GenAIRecord, Queue, ScouterQueue
from scouter.transport import HttpConfig

from .assets.prompts import prompt_state
from .models import Answer, Question

logger = RustyLogger.get_logger(
    LoggingConfig(log_level=LogLevel.Debug),
)


@asynccontextmanager
async def lifespan(app: FastAPI):
    logger.info("Starting up FastAPI app")

    app.state.queue = ScouterQueue.from_path(
        path={"genai": Path("api/assets/genai_drift_profile.json")},
        transport_config=HttpConfig(),
    )
    app.state.prompt_state = prompt_state

    yield

    logger.info("Shutting down FastAPI app")


app = FastAPI(lifespan=lifespan)


@app.post("/predict", response_model=Answer)
async def predict(request: Request, payload: Question) -> Answer:
    # Grab the reformulated prompt and response prompt from the app state
    reformulated_prompt: Prompt = request.app.state.prompt_state.reformulated_prompt
    response_prompt: Prompt = request.app.state.prompt_state.response_prompt
    queue: Queue = request.app.state.queue["genai"]
    agent: Agent = request.app.state.prompt_state.agent

    # Execute reformulated prompt with the user input
    reformulated_query = agent.execute_prompt(
        reformulated_prompt.bind(user_input=payload.question),
    ).response_text()

    # Execute response prompt with the reformulated question
    response = agent.execute_prompt(
        response_prompt.bind(reformulated_query=reformulated_query),
    ).response_text()

    queue.insert(
        GenAIRecord(
            context={
                "user_input": payload.question,
                "reformulated_query": reformulated_query,
                "relevance_response": response,
            },
        )
    )
    assert response is not None
    return Answer(message=response)
