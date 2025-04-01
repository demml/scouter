# pylint: disable=invalid-name
from scouter.alert import (
    AlertThreshold, CustomMetricAlertConfig, OpsGenieDispatchConfig,
)
from scouter.client import ScouterClient
from scouter.drift import (
    CustomDriftProfile,
    CustomMetric,
    CustomMetricDriftConfig,
)
from scouter.types import CommonCrons


def my_custom_metric() -> float:
    return 0.03


if __name__ == "__main__":
    # Specify the alert configuration
    alert_config = CustomMetricAlertConfig(
        dispatch_config=OpsGenieDispatchConfig(team='the-ds-team'), # Notify my team via Opsgenie if drift is detected
        schedule=CommonCrons.EveryWeek # Run drift detection job once weekly
    )

    # Create drift config
    custom_config = CustomMetricDriftConfig(
        name="wine_model",
        repository="wine_model",
        version="0.0.1",
        alert_config=alert_config
    )

    # Create the drift profile
    custom_profile = CustomDriftProfile(
        config=custom_config,
        metrics=[
            CustomMetric(
                name="custom_metric_name",
                value=my_custom_metric(),
                # Alerts if the observed metric value exceeds the baseline.
                alert_threshold=AlertThreshold.Above,
                # If alert_threshold_value isn’t set, any increase triggers an alert.
                alert_threshold_value=0.02
            ),
        ],
    )

    # Register your profile with scouter server
    client = ScouterClient()

    # set_active must be set to True if you want scouter server to run the drift detection job
    client.register_profile(custom_profile, set_active=True)
