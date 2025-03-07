import random
import time

import pandas as pd
from scouter.alert import AlertThreshold, CustomMetricAlertConfig
from scouter.client import (
    BinnedCustomMetrics,
    BinnedPsiFeatureMetrics,
    BinnedSpcFeatureMetrics,
    DriftAlertRequest,
    DriftRequest,
    GetProfileRequest,
    HTTPConfig,
    ProfileStatusRequest,
    ScouterClient,
    TimeInterval,
)
from scouter.drift import (
    CustomMetric,
    CustomMetricDriftConfig,
    Drifter,
    PsiDriftConfig,
    SpcDriftConfig,
)
from scouter.queue import (
    DriftTransportConfig,
    Feature,
    Features,
    KafkaConfig,
    Metric,
    Metrics,
    RabbitMQConfig,
    ScouterQueue,
)
from scouter.types import DriftType

semver = f"{random.randint(0, 10)}.{random.randint(0, 10)}.{random.randint(0, 100)}"


def test_psi_monitor_pandas_http(
    pandas_dataframe: pd.DataFrame,
    psi_drift_config: PsiDriftConfig,
):
    scouter = Drifter()
    client = ScouterClient()

    profile = scouter.create_drift_profile(pandas_dataframe, psi_drift_config)
    client.register_profile(profile)

    config = DriftTransportConfig(id="test", config=HTTPConfig())
    queue = ScouterQueue(config)
    records = pandas_dataframe.to_dict(orient="records")

    for record in records:
        features = Features(
            features=[
                Feature.float("column_0", record["column_0"]),
                Feature.float("column_1", record["column_1"]),
                Feature.float("column_2", record["column_2"]),
            ]
        )
        queue.insert(features)

    queue.flush()

    binned_records: BinnedPsiFeatureMetrics = client.get_binned_drift(
        DriftRequest(
            name=profile.config.name,
            repository=profile.config.repository,
            version=profile.config.version,
            time_interval=TimeInterval.FifteenMinutes,
            max_data_points=1000,
            drift_type=DriftType.Psi,
        )
    )

    assert binned_records is not None


def test_spc_monitor_pandas_kafka(
    pandas_dataframe: pd.DataFrame,
    drift_config: SpcDriftConfig,
):
    scouter = Drifter()
    client = ScouterClient()

    profile = scouter.create_drift_profile(pandas_dataframe, drift_config)
    client.register_profile(profile)

    config = DriftTransportConfig(id="test", config=KafkaConfig())
    queue = ScouterQueue(config)
    records = pandas_dataframe.to_dict(orient="records")

    for record in records:
        features = Features(
            features=[
                Feature.float("column_0", record["column_0"]),
                Feature.float("column_1", record["column_1"]),
                Feature.float("column_2", record["column_2"]),
            ]
        )
        queue.insert(features)

    queue.flush()

    # wait for kafka to process the message
    time.sleep(2)

    binned_records: BinnedSpcFeatureMetrics = client.get_binned_drift(
        DriftRequest(
            name=profile.config.name,
            repository=profile.config.repository,
            version=profile.config.version,
            time_interval=TimeInterval.FifteenMinutes,
            max_data_points=1000,
            drift_type=DriftType.Spc,
        )
    )

    assert len(binned_records.features["column_0"].values) > 0


def test_custom_monitor_pandas_rabbitmq():
    scouter = Drifter()
    client = ScouterClient()

    metrics = [
        CustomMetric(name="mae", value=1, alert_threshold=AlertThreshold.Outside, alert_threshold_value=0.5),
        CustomMetric(name="mape", value=2, alert_threshold=AlertThreshold.Outside, alert_threshold_value=0.5),
    ]
    drift_config = CustomMetricDriftConfig(
        name="test",
        repository="test",
        version=semver,
        alert_config=CustomMetricAlertConfig(schedule="0/15 * * * * * *"),  # every 15 seconds
    )

    profile = scouter.create_drift_profile(data=metrics, config=drift_config)
    client.register_profile(profile)
    config = DriftTransportConfig(id="test", config=RabbitMQConfig())
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

    time.sleep(2)

    # wait for rabbitmq to process the message
    request = DriftRequest(
        name=profile.config.name,
        repository=profile.config.repository,
        version=profile.config.version,
        time_interval=TimeInterval.FifteenMinutes,
        max_data_points=1000,
        drift_type=DriftType.Custom,
    )

    binned_records: BinnedCustomMetrics = client.get_binned_drift(request)  # type: ignore

    assert len(binned_records.metrics["mae"].stats) > 0

    client.update_profile_status(
        ProfileStatusRequest(
            name=profile.config.name, repository=profile.config.repository, version=profile.config.version, active=True
        )
    )

    # wait for alerts to process
    time.sleep(15)  # wait for 11 because background drift task runs every 10 seconds
    alerts = client.get_alerts(
        DriftAlertRequest(
            name=profile.config.name,
            repository=profile.config.repository,
            version=profile.config.version,
        )
    )

    assert len(alerts) > 0


def test_drift_transport_config_no_profile_provided(
    pandas_dataframe: pd.DataFrame,
    psi_drift_config: PsiDriftConfig,
):
    scouter = Drifter()
    client = ScouterClient()

    profile = scouter.create_drift_profile(pandas_dataframe, psi_drift_config)
    client.register_profile(profile)

    config = DriftTransportConfig(
        id="test",
        config=HTTPConfig(),
        drift_profile_request=GetProfileRequest(
            name=profile.config.name,
            repository=profile.config.repository,
            version=profile.config.version,
            drift_type=profile.config.drift_type,
        ),
    )
    queue = ScouterQueue(config)
    assert queue is not None
