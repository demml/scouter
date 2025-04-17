import random
import time

from scouter.alert import AlertThreshold, CustomMetricAlertConfig
from scouter.client import (
    BinnedCustomMetrics,
    DriftAlertRequest,
    DriftRequest,
    GetProfileRequest,
    ProfileStatusRequest,
    ScouterClient,
    TimeInterval,
)
from scouter.drift import CustomMetric, CustomMetricDriftConfig, Drifter
from scouter.queue import (
    DriftTransportConfig,
    Metric,
    Metrics,
    RabbitMQConfig,
    ScouterQueue,
)
from scouter.test import ScouterTestServer
from scouter.types import DriftType

semver = f"{random.randint(0, 10)}.{random.randint(0, 10)}.{random.randint(0, 100)}"


def _test_custom_monitor_pandas_rabbitmq():
    with ScouterTestServer() as _:
        scouter = Drifter()
        client = ScouterClient()

        metrics = [
            CustomMetric(
                name="mae",
                value=1,
                alert_threshold=AlertThreshold.Outside,
                alert_threshold_value=0.5,
            ),
            CustomMetric(
                name="mape",
                value=2,
                alert_threshold=AlertThreshold.Outside,
                alert_threshold_value=0.5,
            ),
        ]
        drift_config = CustomMetricDriftConfig(
            name="test",
            space="test",
            version=semver,
            alert_config=CustomMetricAlertConfig(
                schedule="0/15 * * * * * *",  # every 15 seconds
            ),
        )

        profile = scouter.create_drift_profile(data=metrics, config=drift_config)
        client.register_profile(profile)
        config = DriftTransportConfig(
            id="test",
            config=RabbitMQConfig(),
            drift_profile_request=GetProfileRequest(
                name=profile.config.name,
                version=profile.config.version,
                space=profile.config.space,
                drift_type=profile.config.drift_type,
            ),
        )
        queue = ScouterQueue(config)
        for i in range(0, 30):
            metrics = Metrics(
                metrics=[
                    Metric("mae", i),
                    Metric("mape", i + 1),
                ]
            )
            queue.insert(metrics)
        queue.flush()

        # wait for rabbitmq to process the message
        time.sleep(10)

        request = DriftRequest(
            name=profile.config.name,
            space=profile.config.space,
            version=profile.config.version,
            time_interval=TimeInterval.FifteenMinutes,
            max_data_points=1000,
            drift_type=DriftType.Custom,
        )

        binned_records: BinnedCustomMetrics = client.get_binned_drift(request)  # type: ignore
        assert len(binned_records.metrics["mae"].stats) > 0

        client.update_profile_status(
            ProfileStatusRequest(
                name=profile.config.name,
                space=profile.config.space,
                version=profile.config.version,
                active=True,
            )
        )

        # wait for alerts to process
        # wait for 11 because background drift task runs every 10 seconds
        time.sleep(11)
        alerts = client.get_alerts(
            DriftAlertRequest(
                name=profile.config.name,
                space=profile.config.space,
                version=profile.config.version,
            )
        )

        assert len(alerts) > 0
