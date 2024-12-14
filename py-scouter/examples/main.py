from scouter import (
    AlertCondition,
    AlertDispatchType,
    CustomComparisonMetric,
    CustomMetricBaseAlertConfig,
    CustomMetricDriftConfig,
    CustomMetricEntry,
    CustomThresholdMetric,
    Drifter,
    DriftType,
    CustomComparisonMetricAlertConfig, CustomThresholdMetricAlertConfig, KafkaConfig,
)
from scouter.drift.base import CustomMetricData
from scouter.drift.drifter import Drifter

from scouter.integrations.fastapi import ScouterRouter

from fastapi import FastAPI, Request
import numpy as np
import pandas as pd
from pydantic import BaseModel


class TestResponse(BaseModel):
    message: str


def generate_data() -> pd.DataFrame:
    """Create a fake data frame for testing"""
    n = 10_00

    X_train = np.random.normal(3, 17.0, size=(n, 3))
    col_names = []
    for i in range(1, X_train.shape[1]+1):
        col_names.append(f"feature_{i}")

    X = pd.DataFrame(X_train, columns=col_names)
    # X["col_11"] = np.random.randint(1, 20, size=(n, 1))
    X["target"] = np.random.randint(1, 10, size=(n, 1))

    return X


breakpoint()
config = KafkaConfig(
    topic="testit",
    brokers="localhost:29092",
    raise_on_err=True,
    config={"bootstrap.servers": "localhost:29092"},
)

drifter.generate_alerts()
drifter.compute_drift()

app = FastAPI()
router = ScouterRouter(drift_profile=profile, config=config)

@router.get("/test", response_model=TestResponse)
async def test_route(request: Request) -> TestResponse:
    request.state.scouter_data = {'col_0': 110, 'col_1': 112, 'target': 113}
    return TestResponse(message="success")

app.include_router(router)
