import random
import tempfile
from pathlib import Path

import pandas as pd
from scouter.client import (
    BinnedPsiFeatureMetrics,
    DriftRequest,
    HTTPConfig,
    ScouterClient,
    TimeInterval,
)
from scouter.drift import Drifter, PsiDriftConfig
from scouter.queue import Feature, Features, ScouterQueue
from scouter.types import DriftType

semver = f"{random.randint(0, 10)}.{random.randint(0, 10)}.{random.randint(0, 100)}"


def test_psi_monitor_pandas_http(
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
    for record in records:
        features = Features(
            features=[
                Feature.float("feature_0", record["feature_0"]),
                Feature.float("feature_1", record["feature_1"]),
                Feature.float("feature_2", record["feature_2"]),
            ]
        )
        # 3. Send records to Scouter
        queue["a"].insert(features)

    # 4. Shutdown the queue
    queue.shutdown()

    binned_records: BinnedPsiFeatureMetrics = client.get_binned_drift(
        DriftRequest(
            name=profile.config.name,
            space=profile.config.space,
            version=profile.config.version,
            time_interval=TimeInterval.FifteenMinutes,
            max_data_points=1000,
            drift_type=DriftType.Psi,
        )
    )

    assert binned_records is not None
