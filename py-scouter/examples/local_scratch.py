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
    CustomComparisonMetricAlertConfig, CustomThresholdMetricAlertConfig,
)
from scouter.drift.base import CustomMetricData
from scouter.drift.drifter import Drifter

# from scouter.drift.custom_drifter import CustomMetricData




comparison_metrics = [
    CustomComparisonMetric(
        metric_name="MAE",
        features=[CustomMetricEntry(feature_name="target", metric_value=1.2)],
        alert_config=CustomComparisonMetricAlertConfig(
            base_config=CustomMetricBaseAlertConfig(
                dispatch_type=AlertDispatchType.Console,
                features_to_monitor=["target"]),
            alert_condition=AlertCondition.BELOW,
            alert_boundary=None,
        ),
    ),
    CustomComparisonMetric(
        metric_name="MSE",
        features=[CustomMetricEntry(feature_name="target", metric_value=12.1)],
        alert_config=CustomComparisonMetricAlertConfig(
            base_config=CustomMetricBaseAlertConfig(
                dispatch_type=AlertDispatchType.Console,
                features_to_monitor=["target"]),
            alert_condition=AlertCondition.BELOW,
            alert_boundary=3.0,
        )
    )
]
threshold_metric = CustomThresholdMetric(
    metric_name="KL",
    features=[
        CustomMetricEntry(feature_name="feature1", metric_value=0.1),
        CustomMetricEntry(feature_name="feature2", metric_value=0.11),
        CustomMetricEntry(feature_name="feature3", metric_value=0.05),
    ],
    alert_config=CustomThresholdMetricAlertConfig(
        alert_threshold=0.3,
        base_config=CustomMetricBaseAlertConfig(
            dispatch_type=AlertDispatchType.Console,
            features_to_monitor=["target"]
        )
    )
)
config = CustomMetricDriftConfig(
    repository="scouter",
    name="model",
    version="0.1.0",
    schedule="0 30 12 15 * ? *"
)
drifter = Drifter(DriftType.CUSTOM)
profile = drifter.create_drift_profile(data=CustomMetricData(comparison_metrics=comparison_metrics, threshold_metrics=threshold_metric), config=config)
print(profile)

# from scouter.integrations.fastapi import ScouterRouter
#
# from fastapi import FastAPI, Request
# import numpy as np
# import pandas as pd
# from pydantic import BaseModel


# class TestResponse(BaseModel):
#     message: str
#
#
# # def generate_data() -> pd.DataFrame:
# #     """Create a fake data frame for testing"""
# #     n = 10_00
# #
# #     X_train = np.random.normal(3, 17.0, size=(n, 3))
# #     col_names = []
# #     for i in range(1, X_train.shape[1]+1):
# #         col_names.append(f"feature_{i}")
# #
# #     X = pd.DataFrame(X_train, columns=col_names)
# #     # X["col_11"] = np.random.randint(1, 20, size=(n, 1))
# #     X["target"] = np.random.randint(1, 10, size=(n, 1))
# #
# #     return X


# breakpoint()
# config = KafkaConfig(
#     topic="testit",
#     brokers="localhost:29092",
#     raise_on_err=True,
#     config={"bootstrap.servers": "localhost:29092"},
# )
#
# drifter.generate_alerts()
# drifter.compute_drift()
#
# app = FastAPI()
# router = ScouterRouter(drift_profile=profile, config=config)
#
# @router.get("/test", response_model=TestResponse)
# async def test_route(request: Request) -> TestResponse:
#     request.state.scouter_data = {'col_0': 110, 'col_1': 112, 'target': 113}
#     return TestResponse(message="success")
#
# app.include_router(router)
