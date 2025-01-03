from typing import Optional

import pandas as pd
from scouter import (
    Drifter,
    DriftType,
    Feature,
    Features,
    KafkaConfig,
    MonitorQueue,
    PsiDriftConfig,
    PsiDriftProfile,
    ServerRecords,
    SpcDriftConfig,
    SpcDriftProfile,
)


def test_psi_monitor_pandas(
    pandas_dataframe: pd.DataFrame,
    psi_drift_config: PsiDriftConfig,
    mock_kafka_producer,
    kafka_config: KafkaConfig,
):
    scouter = Drifter(DriftType.Psi)
    profile: PsiDriftProfile = scouter.create_drift_profile(pandas_dataframe, psi_drift_config)

    queue = MonitorQueue(
        drift_profile=profile,
        config=kafka_config,
    )
    records = pandas_dataframe.to_dict(orient="records")

    def return_record(records) -> Optional[ServerRecords]:
        for record in records:
            features = Features(
                features=[
                    Feature.float("column_0", record["column_0"]),
                    Feature.float("column_1", record["column_1"]),
                    Feature.float("column_2", record["column_2"]),
                ]
            )
            drift_map = queue.insert(features)

            if drift_map:
                return drift_map

        return None

    drift_records = return_record(records)
    assert drift_records is not None
    assert len(drift_records.records) == 30

    queue._queueing_strategy.stop_queue_observer()  # type: ignore


def test_spc_monitor_pandas(
    pandas_dataframe: pd.DataFrame,
    drift_config: SpcDriftConfig,
    mock_kafka_producer,
    kafka_config: KafkaConfig,
):
    scouter = Drifter()
    profile: SpcDriftProfile = scouter.create_drift_profile(pandas_dataframe, drift_config)

    queue = MonitorQueue(
        drift_profile=profile,
        config=kafka_config,
    )

    records = pandas_dataframe[0:30].to_dict(orient="records")

    def return_record(records) -> Optional[ServerRecords]:
        for record in records:
            features = Features(
                features=[
                    Feature.float("column_0", record["column_0"]),
                    Feature.float("column_1", record["column_1"]),
                    Feature.float("column_2", record["column_2"]),
                ]
            )
            drift_map = queue.insert(features)

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
            features = Features(
                features=[
                    Feature.string("cat1", record["cat1"]),
                    Feature.float("num1", record["num1"]),
                    Feature.float("num2", record["num2"]),
                    Feature.float("num3", record["num3"]),
                    Feature.string("cat2", record["cat2"]),
                ]
            )
            drift_map = queue.insert(features)

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

    def return_record() -> Optional[ServerRecords]:
        features = Features(
            features=[
                Feature.string("cat1", "7.0"),
                Feature.float("num1", 1.518124333674737),
                Feature.float("num2", 2.974753543708461),
                Feature.float("num3", 3.141546504798932),
                Feature.string("cat3", "2.0"),  # this is missing
            ]
        )
        drift_map = queue.insert(features)

        if drift_map:
            return drift_map

        return None

    records = return_record()
    assert records is None

    records = polars_dataframe_multi_dtype[0:30].to_dicts()  # type: ignore
    records[0]["num1"] = Drifter  # type: ignore

    records = return_record()
    assert records is None
