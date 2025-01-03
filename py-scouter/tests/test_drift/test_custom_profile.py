from scouter import (
    AlertThreshold,
    CustomMetric,
    CustomMetricDriftConfig,
    Drifter,
    DriftType,
)
from scouter._scouter import CustomDriftProfile


def test_custom_profile(custom_metric_drift_config: CustomMetricDriftConfig):
    # create custom metric obj
    accuracy = CustomMetric(
        name="accuracy",
        value=0.75,
        alert_threshold=AlertThreshold.Below,
        alert_threshold_value=0.05,
    )

    # create custom drifter
    drifter = Drifter(DriftType.Custom)

    # create custom drift profile
    profile: CustomDriftProfile = drifter.create_drift_profile(data=accuracy, config=custom_metric_drift_config)
    # assert profile is what we expect
    assert profile.model_dump() == {
        "config": {
            "alert_config": {
                "alert_conditions": {"accuracy": {"alert_threshold": "Below", "alert_threshold_value": 0.05}},
                "dispatch_kwargs": {},
                "dispatch_type": "Slack",
                "schedule": "0 0 * * * *",
            },
            "drift_type": "Custom",
            "name": "test",
            "repository": "test",
            "version": "0.1.0",
        },
        "metrics": {"accuracy": 0.75},
        "scouter_version": "0.3.3",
    }

    # test helper function that allows users to see their metrics in the format they were submitted
    assert profile.custom_metrics[0].name == "accuracy"
    assert profile.custom_metrics[0].value == 0.75
    assert profile.custom_metrics[0].alert_threshold_value == 0.05
    assert profile.custom_metrics[0].alert_threshold == AlertThreshold.Below
