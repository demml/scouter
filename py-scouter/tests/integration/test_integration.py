from scouter import (
    MonitorQueue,
    SpcDriftConfig,
    SpcDriftProfile,
    Drifter,
    KafkaConfig,
    SpcDriftServerRecords,
)
import pandas as pd
from typing import Optional


def test_monitor_pandas(
    pandas_dataframe: pd.DataFrame,
    drift_config: SpcDriftConfig,
):
    scouter = Drifter()
    profile: SpcDriftProfile = scouter.create_drift_profile(
        pandas_dataframe, drift_config
    )

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

    def return_record(records) -> Optional[SpcDriftServerRecords]:
        for record in records:
            drift_map = queue.insert(record)

            if drift_map:
                return drift_map

        return None

    drift_records = return_record(records)
    assert drift_records is not None
    assert len(drift_records.records) == 3

    queue._producer.flush()
