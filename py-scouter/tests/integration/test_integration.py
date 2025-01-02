from typing import Optional

import pandas as pd
from scouter import (
    Drifter,
    DriftType,
    Feature,
    Features,
    KafkaConfig,
    MonitorQueue,
    ServerRecords,
    SpcDriftConfig,
    SpcDriftProfile,
)


def test_monitor_pandas(
    pandas_dataframe: pd.DataFrame,
    drift_config: SpcDriftConfig,
):
    scouter = Drifter(DriftType.Spc)
    profile: SpcDriftProfile = scouter.create_drift_profile(pandas_dataframe, drift_config)

    kafka_config = KafkaConfig(
        topic="scouter_monitoring",
        brokers="localhost:9092",
        raise_on_err=True,
    )

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

    queue.flush()
