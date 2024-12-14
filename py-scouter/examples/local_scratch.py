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
profile = drifter.create_drift_profile(data=CustomMetricData(threshold_metrics=threshold_metric, comparison_metrics=comparison_metrics), config=config)
print(profile)
print(profile.threshold_metrics[0])
