from scouter import (
    AlertCondition,
    AlertDispatchType,
    CustomMetricAlertConfig,
    # CustomComparisonMetric,
    CustomMetricDriftConfig,
    CustomMetricEntry,
    CustomThresholdMetric,
    Drifter,
    DriftType,
)

# comparison_metrics = [
#     CustomComparisonMetric(
#         metric_name="MAE",
#         features=[CustomMetricEntry(feature_name="target", metric_value=1.2)],
#         alert_condition=AlertCondition.BELOW,
#         alert_boundary=None,
#     ),
#     CustomComparisonMetric(
#         metric_name="MSE",
#         features=[CustomMetricEntry(feature_name="target", metric_value=12.1)],
#         alert_condition=AlertCondition.BELOW,
#         alert_boundary=3.0,
#     ),
# ]

threshold_metrics = [
    CustomThresholdMetric(
        metric_name="KL",
        features=[
            CustomMetricEntry(feature_name="feature1", metric_value=0.1),
            CustomMetricEntry(feature_name="feature2", metric_value=0.11),
            CustomMetricEntry(feature_name="feature3", metric_value=0.05),
        ],
        alert_threshold=0.5,
    ),
]


config = CustomMetricDriftConfig(
    repository="scouter",
    name="model",
    version="0.1.0",
    alert_config=CustomMetricAlertConfig(
        dispatch_type=AlertDispatchType.Console, schedule="0 0 9 * * *", features_to_monitor=["feature_1", "feature_2"]
    ),
)
# breakpoint()
# drifter = Drifter.build(DriftType.Custom)
# breakpoint()
# profile = drifter.create_drift_profile(comparison_metrics, threshold_metrics, config)


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
