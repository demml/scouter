import asyncio
from contextlib import asynccontextmanager
from pathlib import Path
from textwrap import dedent

import numpy as np
import pandas as pd
from fastapi import FastAPI, Request
from pydantic import BaseModel
from scouter import HttpConfig, KafkaConfig, Queue, ScouterQueue
from scouter.alert import (
    AlertCondition,
    AlertThreshold,
    GenAIAlertConfig,
    SpcAlertConfig,
)
from scouter.client import ScouterClient
from scouter.drift import (
    ComparisonOperator,
    Drifter,
    GenAIEvalConfig,
    GenAIEvalProfile,
    LLMJudgeTask,
    SpcDriftConfig,
    SpcDriftProfile,
)
from scouter.genai import Agent, Prompt, Provider, Score
from scouter.logging import LoggingConfig, LogLevel, RustyLogger
from scouter.queue import GenAIEvalRecord
from scouter.tracing import (
    BatchConfig,
    TestSpanExporter,
    Tracer,
    get_tracer,
    init_tracer,
    shutdown_tracer,
)
from scouter.transport import GrpcConfig
from scouter.util import FeatureMixin

logger = RustyLogger.get_logger(
    LoggingConfig(log_level=LogLevel.Debug),
)


def create_coherence_evaluation_prompt() -> Prompt:
    message = dedent(
        """
        You will be given a text passage to evaluate for coherence.

        # Task
        Evaluate the coherence of the given text on a scale of 1 to 5, where:
        - 1: Very poor coherence - ideas are disconnected, contradictory, or extremely difficult to follow
        - 2: Poor coherence - some logical connections missing, frequent jumps between ideas
        - 3: Moderate coherence - generally logical flow with occasional unclear transitions
        - 4: Good coherence - clear logical progression with minor coherence issues
        - 5: Excellent coherence - smooth, logical flow with clear connections between all ideas

        # Evaluation Criteria
        Consider the following aspects when evaluating coherence:
        1. **Logical Flow**: Do ideas progress in a logical sequence?
        2. **Transitions**: Are there clear connections between sentences and paragraphs?
        3. **Consistency**: Are the main themes and arguments consistent throughout?
        4. **Clarity**: Is the overall message clear and easy to follow?
        5. **Structure**: Does the text have a clear beginning, middle, and end?

        # Text to Evaluate
        ${response}

        # Instructions
        1. Read the text carefully
        2. Assess each of the coherence criteria above
        3. Provide a coherence score from 1 to 5
        4. Give a brief explanation (2-3 sentences) justifying your score

        # Response Format
        Score: [Your score 1-5]
        Explanation: [Your brief explanation]
    """
    ).strip()

    return Prompt(
        messages=message,
        model="gpt-4o",
        provider="openai",
        output_type=Score,
    )


def generate_data() -> pd.DataFrame:
    """Create a fake data frame for testing"""
    n = 10_000

    X_train = np.random.normal(-4, 2.0, size=(n, 4))

    col_names = []
    for i in range(0, X_train.shape[1]):
        col_names.append(f"feature_{i}")

    X = pd.DataFrame(X_train, columns=col_names)

    return X


def create_and_register_drift_profile(
    client: ScouterClient,
    name: str,
) -> SpcDriftProfile:
    data = generate_data()

    # create drift config (usually associated with a model name, space name, version)
    config = SpcDriftConfig(
        space="scouter",
        name=name,
        version="0.1.0",
        alert_config=SpcAlertConfig(features_to_monitor=data.columns.tolist()),
    )

    # create drifter
    drifter = Drifter()

    # create drift profile
    profile = drifter.create_drift_profile(data, config)
    client.register_profile(profile, True)

    return profile


def create_and_register_genai_drift_profile(
    client: ScouterClient,
    name: str,
) -> GenAIEvalProfile:
    # create drift config (usually associated with a model name, space name, version)
    config = GenAIEvalConfig(
        space="scouter",
        name=name,
        version="0.1.0",
        sample_ratio=1,
        alert_config=GenAIAlertConfig(
            alert_condition=AlertCondition(
                baseline_value=0.80,
                alert_threshold=AlertThreshold.Below,
                delta=0.01,
            )
        ),
    )

    tasks = [
        LLMJudgeTask(
            id="coherence",
            expected_value=4,
            prompt=create_coherence_evaluation_prompt(),
            field_path="score",
            operator=ComparisonOperator.GreaterThanOrEqual,
            description="Evaluate text coherence",
        )
    ]

    # create drifter
    drifter = Drifter()

    # create drift profile
    profile = drifter.create_genai_drift_profile(config=config, tasks=tasks)
    client.register_profile(profile, True)

    return profile


class TestResponse(BaseModel):
    message: str


class PredictRequest(BaseModel, FeatureMixin):
    feature_0: float
    feature_1: float
    feature_2: float
    feature_3: float


class InnerResponse(BaseModel):
    sum: float


class ChatRequest(BaseModel):
    question: str


def create_kafka_app(profile_path: Path) -> FastAPI:
    config = KafkaConfig()
    init_tracer(
        service_name="test-service",
        exporter=TestSpanExporter(),
    )

    @asynccontextmanager
    async def lifespan(app: FastAPI):
        logger.info("Starting up FastAPI app")

        app.state.queue = ScouterQueue.from_path(
            path={"spc": profile_path},
            transport_config=config,
        )
        yield

        logger.info("Shutting down FastAPI app")
        # Shutdown the queue
        app.state.queue.shutdown()
        app.state.queue = None
        shutdown_tracer()

    app = FastAPI(lifespan=lifespan)
    tracer = get_tracer("test-service")

    @app.post("/predict", response_model=TestResponse)
    @tracer.span("predict")
    async def predict(request: Request, payload: PredictRequest) -> TestResponse:
        print(f"Received payload: {request.app.state}")
        request.app.state.queue["spc"].insert(payload.to_features())
        return TestResponse(message="success")

    return app


def create_kafka_genai_app(profile_path: Path) -> FastAPI:
    config = KafkaConfig()

    @asynccontextmanager
    async def lifespan(app: FastAPI):
        logger.info("Starting up FastAPI app")

        app.state.agent = Agent(
            system_instruction="You are a helpful assistant",
            provider=Provider.OpenAI,
        )

        app.state.prompt = Prompt(
            messages="Answer the following question and provide a response with a score and reason: ${question}",
            model="gpt-4o",
            provider="openai",
            output_type=Score,
        )

        app.state.queue = ScouterQueue.from_path(
            path={"genai": profile_path},
            transport_config=config,
        )
        yield

        logger.info("Shutting down FastAPI app")
        # Shutdown the queue
        app.state.queue.shutdown()
        app.state.queue = None

    app = FastAPI(lifespan=lifespan)

    @app.post("/chat", response_model=TestResponse)
    async def chat(request: Request, payload: ChatRequest) -> TestResponse:
        queue: Queue = request.app.state.queue["genai"]

        agent: Agent = request.app.state.agent
        prompt: Prompt = request.app.state.prompt

        # Create an GenAIEvalRecord from the payload
        bound_prompt = prompt.bind(question=payload.question)

        response = agent.execute_prompt(prompt=bound_prompt)

        queue.insert(
            GenAIEvalRecord(
                context={
                    "input": bound_prompt.messages[0].text(),
                    "response": response.response_text(),
                },
            )
        )
        return TestResponse(message="success")

    @app.post("/flush", response_model=TestResponse)
    async def flush(request: Request) -> TestResponse:
        queue: ScouterQueue = request.app.state.queue
        queue.shutdown()
        return TestResponse(message="flushed")

    return app


def create_http_app(profile_path: Path) -> FastAPI:
    config = HttpConfig()
    init_tracer(
        service_name="test-service",
        exporter=TestSpanExporter(batch_export=True),
        batch_config=BatchConfig(scheduled_delay_ms=200),
    )
    tracer = get_tracer("test-service")

    @asynccontextmanager
    async def lifespan(app: FastAPI):
        logger.info("Starting up FastAPI app")

        app.state.queue = ScouterQueue.from_path(
            path={"spc": profile_path},
            transport_config=config,
        )
        yield

        logger.info("Shutting down FastAPI app")
        # Shutdown the queue
        app.state.queue = None
        shutdown_tracer()

    app = FastAPI(lifespan=lifespan)

    @tracer.span("nested1")
    async def nested1(feature_1: float, feature_2: float) -> InnerResponse:
        await asyncio.sleep(0.05)
        return InnerResponse(sum=feature_1 + feature_2)

    @tracer.span("nested2")
    async def nested2(feature_1: float, feature_2: float) -> InnerResponse:
        await asyncio.sleep(0.05)
        return InnerResponse(sum=feature_1 + feature_2)

    @app.post("/predict", response_model=TestResponse)
    @tracer.span("predict", baggage=[{"zoo": "bat"}], tags=[{"foo": "bar"}])
    async def predict(request: Request, payload: PredictRequest) -> TestResponse:
        await nested1(payload.feature_1, payload.feature_2)
        await nested2(payload.feature_3, payload.feature_0)
        request.app.state.queue["spc"].insert(payload.to_features())
        return TestResponse(message="success")

    return app


def create_tracing_genai_app(tracer: Tracer, profile_path: Path) -> FastAPI:
    config = GrpcConfig()

    @asynccontextmanager
    async def lifespan(app: FastAPI):
        logger.info("Starting up FastAPI app")

        app.state.agent = Agent(
            system_instruction="You are a helpful assistant",
            provider=Provider.OpenAI,
        )

        app.state.prompt = Prompt(
            messages="Answer the following question and provide a response with a score and reason: ${question}",
            model="gpt-4o",
            provider="openai",
            output_type=Score,
        )

        queue = ScouterQueue.from_path(
            path={"genai": profile_path},
            transport_config=config,
        )
        tracer.set_scouter_queue(queue)
        yield

        logger.info("Shutting down FastAPI app")
        # Shutdown the queue
        queue.shutdown()

    app = FastAPI(lifespan=lifespan)

    @app.post("/chat", response_model=TestResponse)
    async def chat(request: Request, payload: ChatRequest) -> TestResponse:
        with tracer.start_as_current_span("genai_service") as active_span:
            agent: Agent = request.app.state.agent
            prompt: Prompt = request.app.state.prompt

            # Create an GenAIEvalRecord from the payload
            bound_prompt = prompt.bind(question=payload.question)

            response = agent.execute_prompt(prompt=bound_prompt)

            active_span.add_queue_item(
                alias="genai",
                item=GenAIEvalRecord(
                    context={
                        "input": bound_prompt.messages[0].text(),
                        "response": response.response_text(),
                    },
                ),
            )

            return TestResponse(message="success")

    return app
