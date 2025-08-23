import random
import tempfile
from pathlib import Path

import pandas as pd
from scouter.client import HTTPConfig, ScouterClient
from scouter.drift import Drifter, PsiDriftConfig
from scouter.mock import MockConfig
from scouter.queue import Feature, Features, ScouterQueue

semver = f"{random.randint(0, 10)}.{random.randint(0, 10)}.{random.randint(0, 100)}"


def test_mock_config(
    mock_environment,  # this will mock the HTTPConfig import
    http_scouter_server,
    pandas_dataframe: pd.DataFrame,
    psi_drift_config: PsiDriftConfig,
):
    scouter = Drifter()
    client = ScouterClient()
    profile = scouter.create_drift_profile(pandas_dataframe, psi_drift_config)
    client.register_profile(profile)

    with tempfile.TemporaryDirectory() as temp_dir:
        path = Path(temp_dir) / "profile.json"
        profile.save_to_json(path)

        ### Workflow
        # 1. Create a ScouterQueue from path
        queue = ScouterQueue.from_path({"a": path}, HTTPConfig())

    # 2. Simulate records
    records = pandas_dataframe.to_dict(orient="records")
    for record in records[:10]:
        features = Features(
            features=[
                Feature.float(column_name, record[column_name])
                for column_name in pandas_dataframe.columns
            ]
        )
        # 3. Send records to Scouter
        queue["a"].insert(features)

    # 4. Shutdown the queue
    queue.shutdown()

    assert isinstance(queue.transport_config, MockConfig)
    a


def _test_mock_config_kwargs():
    MockConfig(
        kafka_brokers="localhost:9092",
        kafka_topic="test_topic",
        kafka_compression_type="gzip",
    )
