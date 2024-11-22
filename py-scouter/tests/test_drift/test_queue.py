from scouter import (
    MonitorQueue,
    SpcDriftConfig,
    SpcDriftProfile,
    Drifter,
    KafkaConfig,
    ServerRecords,
    PsiDriftConfig,
    PsiDriftProfile,
    DriftType
)
from typing import Optional
import pandas as pd


def test_psi_monitor_pandas(
        pandas_dataframe: pd.DataFrame,
        psi_drift_config: PsiDriftConfig,
        mock_kafka_producer,
        kafka_config: KafkaConfig,
):
    scouter = Drifter(DriftType.PSI)
    profile: PsiDriftProfile = scouter.create_drift_profile(
        pandas_dataframe, psi_drift_config
    )

    queue = MonitorQueue(
        drift_profile=profile,
        config=kafka_config,
    )
    records = pandas_dataframe.to_dict(orient="records")

    def return_record(records) -> Optional[ServerRecords]:
        for record in records:
            drift_map = queue.insert(record)

            if drift_map:
                return drift_map

        return None

    drift_records = return_record(records)
    assert drift_records is not None
    assert len(drift_records.records) == 30


def test_spc_monitor_pandas(
    pandas_dataframe: pd.DataFrame,
    drift_config: SpcDriftConfig,
    mock_kafka_producer,
    kafka_config: KafkaConfig,
):
    scouter = Drifter()
    profile: SpcDriftProfile = scouter.create_drift_profile(
        pandas_dataframe, drift_config
    )

    queue = MonitorQueue(
        drift_profile=profile,
        config=kafka_config,
    )

    records = pandas_dataframe[0:30].to_dict(orient="records")

    def return_record(records) -> Optional[ServerRecords]:
        for record in records:
            drift_map = queue.insert(record)

            if drift_map:
                return drift_map

        return None

    drift_records = return_record(records)
    assert drift_records is not None
    assert len(drift_records.records) == 3


def test_spc_monitor_polar_multitype(
    polars_dataframe_multi_dtype: pd.DataFrame,
    drift_config: SpcDriftConfig,
    mock_kafka_producer,
    kafka_config: KafkaConfig,
):
    scouter = Drifter()
    profile: SpcDriftProfile = scouter.create_drift_profile(
        polars_dataframe_multi_dtype,
        drift_config,
    )

    queue = MonitorQueue(
        drift_profile=profile,
        config=kafka_config,
    )

    records = polars_dataframe_multi_dtype[0:30].to_dicts()  # type: ignore

    def return_record(records) -> Optional[ServerRecords]:
        for record in records:
            drift_map = queue.insert(record)

            if drift_map:
                return drift_map
        return None

    drift_records = return_record(records)
    assert drift_records is not None
    assert len(drift_records.records) == 5


def test_spc_queue_fail(
    polars_dataframe_multi_dtype: pd.DataFrame,
    drift_config: SpcDriftConfig,
    mock_kafka_producer,
    kafka_config: KafkaConfig,
):
    scouter = Drifter()
    profile: SpcDriftProfile = scouter.create_drift_profile(
        polars_dataframe_multi_dtype,
        drift_config,
    )

    queue = MonitorQueue(
        drift_profile=profile,
        config=kafka_config,
    )

    records = [
        {
            "cat1": "7.0",
            "num1": 1.518124333674737,
            "num2": 2.974753543708461,
            "num3": 3.141546504798932,
            "cat3": "2.0",  # this is missing
        }
    ]

    def return_record(records):
        for record in records:
            drift_map = queue.insert(record)

            if drift_map:
                return drift_map
    records = return_record(records)
    assert records is None

    records = polars_dataframe_multi_dtype[0:30].to_dicts()
    records[0]["num1"] = Drifter  # this should fail

    records = return_record(records)
    assert records is None
