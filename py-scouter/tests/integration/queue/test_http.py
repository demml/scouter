import random
import tempfile
import time
from pathlib import Path
from typing import Dict, Union, cast

import pandas as pd
from scouter.client import (
    BinnedPsiFeatureMetrics,
    DriftRequest,
    ScouterClient,
    TimeInterval,
)
from scouter.drift import Drifter, PsiDriftConfig
from scouter.queue import Features, ScouterQueue
from scouter.transport import HttpConfig
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
        queue = ScouterQueue.from_path({"a": path}, HttpConfig())

    # 2. Simulate records
    records = pandas_dataframe.to_dict(orient="records")

    for record in records:
        features = Features(
            features=cast(
                Dict[str, Union[float, int, str]],
                record,
            )
        )
        # 3. Send records to Scouter
        queue["a"].insert(features)

    # 4. Shutdown the queue
    queue.shutdown()

    time.sleep(5)  # Wait for the data to be processed

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
