import random
import tempfile
import time
from pathlib import Path

from scouter import (  # type: ignore
    CustomMetric,
    CustomMetricDriftConfig,
    Drifter,
    Metric,
    Metrics,
    RabbitMQConfig,
    ScouterQueue,
)
from scouter.alert import AlertThreshold, CustomMetricAlertConfig
from scouter.client import (
    BinnedCustomMetrics,
    DriftAlertRequest,
    DriftRequest,
    ProfileStatusRequest,
    ScouterClient,
    TimeInterval,
)
from scouter.types import DriftType

semver = f"{random.randint(0, 10)}.{random.randint(0, 10)}.{random.randint(0, 100)}"


def test_custom_monitor_pandas_rabbitmq(rabbitmq_scouter_server):
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
        sample_size=5,
        alert_config=CustomMetricAlertConfig(
            schedule="*/5 * * * * *",  # every 5
        ),
    )

    profile = scouter.create_drift_profile(data=metrics, config=drift_config)
    client.register_profile(profile)

    with tempfile.TemporaryDirectory() as temp_dir:
        path = Path(temp_dir) / "profile.json"
        profile.save_to_json(path)
        queue = ScouterQueue.from_path({"a": path}, RabbitMQConfig())

    for i in range(0, 100):
        metrics = Metrics(
            metrics=[
                Metric("mae", i),
                Metric("mape", i + 1),
            ]
        )
        queue["a"].insert(metrics)
    queue.shutdown()

    # wait for rabbitmq to process the message
    time.sleep(5)

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
    time.sleep(5)
    alerts = client.get_alerts(
        DriftAlertRequest(
            name=profile.config.name,
            space=profile.config.space,
            version=profile.config.version,
        )
    )

    assert len(alerts) > 0
