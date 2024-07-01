from scouter import MonitorQueue, DriftConfig, DriftProfile, Drifter, KafkaConfig
import pandas as pd


def test_monitor_pandas(
    pandas_dataframe: pd.DataFrame,
    monitor_config: DriftConfig,
    mock_kafka_producer,
):
    scouter = Drifter()
    profile: DriftProfile = scouter.create_drift_profile(
        pandas_dataframe, monitor_config
    )

    kafka_config = KafkaConfig(
        topic="test-topic",
        brokers="localhost:9092",
        raise_on_err=True,
    )

    queue = MonitorQueue(
        drift_profile=profile,
        config=kafka_config,
    )

    records = pandas_dataframe[0:30].to_dict(orient="records")

    def return_record(records):
        for record in records:
            drift_map = queue.insert(record)

            if drift_map:
                return drift_map

    records = return_record(records)
    assert len(records) == 3
