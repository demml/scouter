import random
import tempfile
import time
from pathlib import Path

import pandas as pd
from scouter.client import (
    BinnedSpcFeatureMetrics,
    DriftRequest,
    ScouterClient,
    TimeInterval,
)
from scouter.drift import Drifter, SpcDriftConfig
from scouter.queue import Feature, Features, KafkaConfig, ScouterQueue
from scouter.types import DriftType

semver = f"{random.randint(0, 10)}.{random.randint(0, 10)}.{random.randint(0, 100)}"


def test_spc_monitor_pandas_kafka(
    kafka_scouter_server,
    pandas_dataframe: pd.DataFrame,
    drift_config: SpcDriftConfig,
):
    scouter = Drifter()
    client = ScouterClient()

    profile = scouter.create_drift_profile(pandas_dataframe, drift_config)
    client.register_profile(profile)

    with tempfile.TemporaryDirectory() as temp_dir:
        path = Path(temp_dir) / "profile.json"
        profile.save_to_json(path)
        queue = ScouterQueue.from_path({"a": path}, KafkaConfig())

    records = pandas_dataframe.to_dict(orient="records")

    for record in records:
        features = Features(
            features=[
                Feature.float("column_0", record["column_0"]),
                Feature.float("column_1", record["column_1"]),
                Feature.float("column_2", record["column_2"]),
            ]
        )
        queue["a"].insert(features)

    queue.shutdown()
    time.sleep(10)

    binned_records: BinnedSpcFeatureMetrics = client.get_binned_drift(
        DriftRequest(
            name=profile.config.name,
            space=profile.config.space,
            version=profile.config.version,
            time_interval=TimeInterval.FifteenMinutes,
            max_data_points=1000,
            drift_type=DriftType.Spc,
        )
    )

    assert len(binned_records.features["column_0"].values) > 0
