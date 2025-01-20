import random
import time

import pandas as pd
from scouter import (
    AlertThreshold,
    CommonCrons,
    CustomMetric,
    CustomMetricAlertConfig,
    CustomMetricDriftConfig,
    CustomMetricServerRecord,
    Drifter,
    DriftType,
    Feature,
    Features,
    PsiDriftConfig,
    RecordType,
    ServerRecord,
    ServerRecords,
    SpcDriftConfig,
)
from scouter.client import (
    BinnedCustomMetrics,
    BinnedPsiFeatureMetrics,
    BinnedSpcFeatureMetrics,
    DriftRequest,
    HTTPConfig,
    ProfileStatusRequest,
    ScouterClient,
    TimeInterval,
)
from scouter.logging import LoggingConfig, LogLevel, RustyLogger
from scouter.queue import KafkaConfig, RabbitMQConfig, ScouterProducer, ScouterQueue

RustyLogger.setup_logging(
    LoggingConfig(log_level=LogLevel.Debug),
)


semver = f"{random.randint(0, 10)}.{random.randint(0, 10)}.{random.randint(0, 100)}"


def test_psi_monitor_pandas_http(
    pandas_dataframe: pd.DataFrame,
    psi_drift_config: PsiDriftConfig,
):
    scouter = Drifter()
    client = ScouterClient()

    profile = scouter.create_drift_profile(pandas_dataframe, psi_drift_config)
    client.register_profile(profile)

    queue = ScouterQueue(drift_profile=profile, config=HTTPConfig())
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

    queue = ScouterQueue(drift_profile=profile, config=KafkaConfig())
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
        alert_config=CustomMetricAlertConfig(schedule=CommonCrons.Every1Minute.cron),
    )

    profile = scouter.create_drift_profile(data=metrics, config=drift_config)
    client.register_profile(profile)
    producer = ScouterProducer(config=RabbitMQConfig())

    for i in range(10, 20):

        record = CustomMetricServerRecord(
            repository="test",
            name="test",
            version=semver,
            metric="mae",
            value=i,
        )

        producer.publish(
            message=ServerRecords(
                records=[ServerRecord(record)],
                record_type=RecordType.Custom,
            )
        )

    producer.flush()

    # wait for rabbitmq to process the message
    time.sleep(2)

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
