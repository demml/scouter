from scouter import Drifter
import numpy as np
from pathlib import Path
import pandas as pd
from numpy.typing import NDArray
import pytest
import polars as pl
from scouter._scouter import PsiDriftProfile, PsiDriftConfig, DriftType
from tempfile import TemporaryDirectory
from scouter.utils.types import Constants


def test_drift_f64(array: NDArray, psi_drift_config: PsiDriftConfig):
    drifter = Drifter(DriftType.PSI)
    profile: PsiDriftProfile = drifter.create_drift_profile(array, psi_drift_config)

    # assert features are relatively centered

    assert profile.features["feature_0"].bins[0].upper_limit == pytest.approx(1.1, 0.1)
    assert profile.features["feature_1"].bins[0].upper_limit == pytest.approx(2.1, 0.1)
    assert profile.features["feature_2"].bins[0].upper_limit == pytest.approx(3.1, 0.1)

    ## save profile to json
    with TemporaryDirectory() as temp_dir:
        profile.save_to_json(Path(temp_dir) / "profile.json")
        assert (Path(temp_dir) / "profile.json").exists()

        with open(Path(temp_dir) / "profile.json", "r") as f:
            PsiDriftProfile.model_validate_json(f.read())
        _ = drifter.compute_drift(array, profile)


# profile.update_config_args(repository="repo1", name="name1")

# assert profile.config.name == "name1"
# assert profile.config.repository == "repo1"
