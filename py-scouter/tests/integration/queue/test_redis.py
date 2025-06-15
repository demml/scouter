import random
import tempfile
from pathlib import Path

import pandas as pd
import polars as pl
from scouter import (  # type: ignore
    Drifter,
    Feature,
    Features,
    PsiDriftConfig,
    RedisConfig,
    ScouterQueue,
)
from scouter.client import (
    BinnedPsiFeatureMetrics,
    DriftRequest,
    ScouterClient,
    TimeInterval,
)
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
        queue = ScouterQueue.from_path({"a": path}, RedisConfig())

    # 2. Simulate records
    records = pandas_dataframe.to_dict(orient="records")
    for record in records:
        features = Features(
            features=[Feature.float(column_name, record[column_name]) for column_name in pandas_dataframe.columns]
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


def test_psi_monitor_polars_categorical_http(
    http_scouter_server,
    polars_dataframe_multi_dtype: pl.DataFrame,
    psi_drift_config_with_categorical_features: PsiDriftConfig,
):
    scouter = Drifter()
    client = ScouterClient()

    profile = scouter.create_drift_profile(
        polars_dataframe_multi_dtype,
        psi_drift_config_with_categorical_features,
    )
    client.register_profile(profile)

    with tempfile.TemporaryDirectory() as temp_dir:
        path = Path(temp_dir) / "profile.json"
        profile.save_to_json(path)

        ### Workflow
        # 1. Create a ScouterQueue from path
        queue = ScouterQueue.from_path({"a": path}, RedisConfig())

    # 2. Simulate records
    records = polars_dataframe_multi_dtype.to_dicts()
    categorical_features = psi_drift_config_with_categorical_features.categorical_features

    for record in records:
        keys = record.keys()
        features = []

        for key in keys:
            if key in categorical_features:
                features.append(Feature.categorical(key, record[key]))
            else:
                features.append(Feature.float(key, record[key]))

        features = Features(features=features)
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
