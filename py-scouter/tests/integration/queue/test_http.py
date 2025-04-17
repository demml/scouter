import random

import pandas as pd
from scouter.client import (
    BinnedPsiFeatureMetrics,
    DriftRequest,
    GetProfileRequest,
    HTTPConfig,
    ScouterClient,
    TimeInterval,
)
from scouter.drift import Drifter, PsiDriftConfig
from scouter.queue import DriftTransportConfig, Feature, Features, ScouterQueue
from scouter.test import ScouterTestServer
from scouter.types import DriftType

semver = f"{random.randint(0, 10)}.{random.randint(0, 10)}.{random.randint(0, 100)}"


def test_psi_monitor_pandas_http(
    pandas_dataframe: pd.DataFrame,
    psi_drift_config: PsiDriftConfig,
):
    with ScouterTestServer() as _:
        scouter = Drifter()
        client = ScouterClient()

        profile = scouter.create_drift_profile(pandas_dataframe, psi_drift_config)
        client.register_profile(profile)

        config = DriftTransportConfig(
            id="test",
            config=HTTPConfig(),
            drift_profile_request=GetProfileRequest(
                name=profile.config.name,
                version=profile.config.version,
                space=profile.config.space,
                drift_type=profile.config.drift_type,
            ),
        )
        queue = ScouterQueue(config)
        records = pandas_dataframe.to_dict(orient="records")

        for record in records:
            features = Features(
                features=[
                    Feature.float("column_0", record["column_0"]),
                    Feature.float("column_1", record["column_1"]),
                    Feature.float("column_2", record["column_2"]),
                ]
            )
            queue.insert(features)

        queue.flush()

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
