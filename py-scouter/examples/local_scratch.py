from scouter import (
    AlertCondition,
    AlertDispatchType,
    CustomMetricDriftConfig,
    DriftType, CustomMetric, CustomMetricAlertConfig, Drifter, CustomMetricAlertCondition,
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
alert_config=CustomMetricAlertConfig(schedule="0 0 * * * *", dispatch_type=AlertDispatchType.Console)
alert_config.alert_conditions = {'steve': CustomMetricAlertCondition()}
config = CustomMetricDriftConfig(
    repository="scouter",
    name="model",
    version="0.1.0",
    alert_config=
)
drifter = Drifter(DriftType.CUSTOM)
profile = drifter.create_drift_profile(data=[mse, mae], config=config)
print(profile.model_dump_json())
