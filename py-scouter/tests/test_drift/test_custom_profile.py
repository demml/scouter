from scouter import Drifter  # type: ignore
from scouter.alert import AlertThreshold
from scouter.drift import CustomDriftProfile, CustomMetric, CustomMetricDriftConfig


def test_custom_profile(custom_metric_drift_config: CustomMetricDriftConfig):
    # create custom metric obj
    accuracy = CustomMetric(
        name="accuracy",
        value=0.75,
        alert_threshold=AlertThreshold.Below,
        alert_threshold_value=0.05,
    )

    # create custom drifter
    drifter = Drifter()

    # create custom drift profile
    profile: CustomDriftProfile = drifter.create_drift_profile(data=accuracy, config=custom_metric_drift_config)
    # assert profile is what we expect
    assert profile.model_dump()["config"] == {
        "alert_config": {
            "alert_conditions": {"accuracy": {"alert_threshold": "Below", "alert_threshold_value": 0.05}},
            "dispatch_config": {"Slack": {"channel": "test_channel"}},
            "schedule": "0 0 * * * *",
        },
        "drift_type": "Custom",
        "name": "test",
        "repository": "test",
        "sample": True,
        "sample_size": 25,
        "version": "0.1.0",
    }

    assert profile.model_dump()["metrics"] == {"accuracy": 0.75}

    # test helper function that allows users to see their metrics in the format they were submitted
    assert profile.custom_metrics[0].name == "accuracy"
    assert profile.custom_metrics[0].value == 0.75
    assert profile.custom_metrics[0].alert_threshold_value == 0.05
    assert profile.custom_metrics[0].alert_threshold == AlertThreshold.Below
