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


def test_monitor_polar_multitype(
    polars_dataframe_multi_dtype: pd.DataFrame,
    monitor_config: DriftConfig,
    mock_kafka_producer,
):
    scouter = Drifter()
    profile: DriftProfile = scouter.create_drift_profile(
        polars_dataframe_multi_dtype,
        monitor_config,
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

    records = polars_dataframe_multi_dtype[0:30].to_dicts()  # type: ignore

    def return_record(records):
        for record in records:
            drift_map = queue.insert(record)

            if drift_map:
                return drift_map

    records = return_record(records)
    assert len(records) == 5


def test_queue_fail(
    polars_dataframe_multi_dtype: pd.DataFrame,
    monitor_config: DriftConfig,
    mock_kafka_producer,
):
    scouter = Drifter()
    profile: DriftProfile = scouter.create_drift_profile(
        polars_dataframe_multi_dtype,
        monitor_config,
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

    records = [
        [
            {
                "cat1": "7.0",
                "num1": 1.518124333674737,
                "num2": 2.974753543708461,
                "num3": 3.141546504798932,
                "cat3": "2.0",  # this is missing
            }
        ]
    ]

    def return_record(records):
        for record in records:
            drift_map = queue.insert(record)

            if drift_map:
                return drift_map

    records = return_record(records)
    assert records is None

    records = [
        [
            {
                "cat1": "7.0",
                "num1": Drifter,  # this should fail
                "num2": 2.974753543708461,
                "num3": 3.141546504798932,
                "cat2": "2.0",
            }
        ]
    ]

    records = return_record(records)
    assert records is None
