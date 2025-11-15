# pylint: disable=invalid-name
from contextlib import asynccontextmanager
from pathlib import Path

from fastapi import FastAPI, Request
from scouter.transport import HTTPConfig
from scouter.llm import Agent, Prompt
from scouter.logging import LoggingConfig, LogLevel, RustyLogger
from scouter.queue import LLMRecord, Queue, ScouterQueue

from .assets.prompts import prompt_state
from .models import Answer, Question

logger = RustyLogger.get_logger(
    LoggingConfig(log_level=LogLevel.Debug),
)


@asynccontextmanager
async def lifespan(app: FastAPI):
    logger.info("Starting up FastAPI app")

    app.state.queue = ScouterQueue.from_path(
        path={"llm": Path("api/assets/llm_drift_profile.json")},
        transport_config=HTTPConfig(),
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
    queue: Queue = request.app.state.queue["llm"]
    agent: Agent = request.app.state.prompt_state.agent

    # Execute reformulated prompt with the user input
    reformulated_query: str = agent.execute_prompt(
        reformulated_prompt.bind(user_input=payload.question),
    ).result

    # Execute response prompt with the reformulated question
    response: str = agent.execute_prompt(
        response_prompt.bind(reformulated_query=reformulated_query),
    ).result

    queue.insert(
        LLMRecord(
            context={
                "user_input": payload.question,
                "reformulated_query": reformulated_query,
                "relevance_response": response,
            },
        )
    )
    return Answer(message=response)
