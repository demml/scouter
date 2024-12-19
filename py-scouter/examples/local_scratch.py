from scouter import (
    AlertCondition,
    AlertDispatchType,
    CustomMetricDriftConfig,
    DriftType, CustomMetric, CustomMetricAlertConfig, Drifter,
)
mae = CustomMetric(
    name="MAE",
    value = 12.0,
    alert_condition=AlertCondition.BELOW,
    alert_boundary=2.0,
)

mse = CustomMetric(
    name="MSE",
    value = 12.0,
    alert_condition=AlertCondition.BELOW,
    alert_boundary=None,
)
config = CustomMetricDriftConfig(
    repository="scouter",
    name="model",
    version="0.1.0",
    alert_config=CustomMetricAlertConfig(schedule="0 0 * * * *", dispatch_type=AlertDispatchType.Console)
)
drifter = Drifter(DriftType.CUSTOM)
profile = drifter.create_drift_profile(data=[mse, mae], config=config)
print(profile.model_dump_json())
