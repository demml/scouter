from scouter import Drifter
import numpy as np
from pathlib import Path
import pandas as pd
from numpy.typing import NDArray
import pytest
import polars as pl
from scouter._scouter import PsiDriftProfile, PsiDriftConfig
from tempfile import TemporaryDirectory
from scouter.utils.types import Constants


def test_drift_f64(array: NDArray, drift_config: SpcDriftConfig):
    drifter = Drifter()
    profile: SpcDriftProfile = drifter.create_drift_profile(array, drift_config)

    # assert features are relatively centered
    assert profile.features["feature_0"].center == pytest.approx(1.5, 0.1)
    assert profile.features["feature_1"].center == pytest.approx(2.5, 0.1)
    assert profile.features["feature_2"].center == pytest.approx(3.5, 0.1)

    # save profile to json
    with TemporaryDirectory() as temp_dir:
        profile.save_to_json(Path(temp_dir) / "profile.json")
        assert (Path(temp_dir) / "profile.json").exists()

        # test loading from json file
        with open(Path(temp_dir) / "profile.json", "r") as f:
            SpcDriftProfile.model_validate_json(f.read())

    _ = drifter.compute_drift(array, profile)

    profile.update_config_args(repository="repo1", name="name1")

    assert profile.config.name == "name1"
    assert profile.config.repository == "repo1"
