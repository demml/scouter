from pathlib import Path

import pandas as pd
from scouter.client import GetProfileRequest, ScouterClient
from scouter.drift import Drifter, SpcDriftConfig


def test_profile_download(
    http_scouter_server,
    tmp_path: Path,
    pandas_dataframe: pd.DataFrame,
    drift_config: SpcDriftConfig,
):
    scouter = Drifter()
    client = ScouterClient()

    profile = scouter.create_drift_profile(pandas_dataframe, drift_config)
    client.register_profile(profile)

    profile_request = GetProfileRequest(
        name=profile.config.name,
        space=profile.config.space,
        version=profile.config.version,
        drift_type=profile.config.drift_type,
    )
    save_path = tmp_path / "profile.json"
    client.download_profile(profile_request, save_path)

    assert save_path.exists()


def test_profile_versions(
    http_scouter_server,
    pandas_dataframe: pd.DataFrame,
    drift_config: SpcDriftConfig,
):
    scouter = Drifter()
    client = ScouterClient()

    profile = scouter.create_drift_profile(pandas_dataframe, drift_config)
    client.register_profile(profile)

    assert profile.config.version == "0.1.0"

    client.register_profile(profile)

    assert profile.config.version == "0.2.0"
    profile
