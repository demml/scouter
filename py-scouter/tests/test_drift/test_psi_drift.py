from pathlib import Path
from tempfile import TemporaryDirectory

import pandas as pd
import polars as pl
import pytest
from numpy.typing import NDArray
from scouter.drift import (
    Drifter,
    EqualWidthBinning,
    PsiDriftConfig,
    PsiDriftProfile,
    SquareRoot,
)


def test_drift_f64(array: NDArray, psi_drift_config: PsiDriftConfig):
    drifter = Drifter()
    profile: PsiDriftProfile = drifter.create_drift_profile(array, psi_drift_config)

    # assert features are relatively centered

    assert profile.features["feature_0"].bins[0].upper_limit == pytest.approx(1.1, 0.1)
    assert profile.features["feature_1"].bins[0].upper_limit == pytest.approx(2.1, 0.1)
    assert profile.features["feature_2"].bins[0].upper_limit == pytest.approx(3.1, 0.1)

    ## save profile to json
    with TemporaryDirectory() as temp_dir:
        path = Path(temp_dir) / "profile.json"
        profile.save_to_json(path)
        assert (Path(temp_dir) / "profile.json").exists()

        with open(Path(temp_dir) / "profile.json", "r") as f:
            PsiDriftProfile.model_validate_json(f.read())
        _ = drifter.compute_drift(array, profile)

        PsiDriftProfile.from_file(path)

    profile.update_config_args(space="repo1", name="name1")

    assert profile.config.name == "name1"
    assert profile.config.space == "repo1"


def test_psi_drift_f32(array: NDArray, psi_drift_config: PsiDriftConfig):
    array = array.astype("float32")
    scouter = Drifter()
    profile: PsiDriftProfile = scouter.create_drift_profile(array, psi_drift_config)

    # assert features are relatively centered
    assert profile.features["feature_0"].bins[0].upper_limit == pytest.approx(1.1, 0.1)
    assert profile.features["feature_1"].bins[0].upper_limit == pytest.approx(2.1, 0.1)
    assert profile.features["feature_2"].bins[0].upper_limit == pytest.approx(3.1, 0.1)

    _ = scouter.compute_drift(array, profile)


def test_only_string_drift_categorical_psi(
    pandas_categorical_dataframe: pd.DataFrame,
    psi_drift_config_with_categorical_features: PsiDriftConfig,
):
    drifter = Drifter()

    profile: PsiDriftProfile = drifter.create_drift_profile(
        pandas_categorical_dataframe,
        psi_drift_config_with_categorical_features,
    )

    drift_map = drifter.compute_drift(pandas_categorical_dataframe, profile)

    assert len(drift_map.features) == 3


def test_only_string_profile_psi(
    pandas_categorical_dataframe: pd.DataFrame,
    psi_drift_config: PsiDriftConfig,
):
    drifter = Drifter()

    profile: PsiDriftProfile = drifter.create_drift_profile(
        pandas_categorical_dataframe,
        psi_drift_config,
    )

    drift_map = drifter.compute_drift(pandas_categorical_dataframe, profile)

    assert len(drift_map.features) == 3


def test_data_pyarrow_mixed_type(
    polars_dataframe_multi_dtype: pl.DataFrame,
    psi_drift_config_with_categorical_features: PsiDriftConfig,
):
    arrow_table = polars_dataframe_multi_dtype.to_arrow()

    drifter = Drifter()

    profile: PsiDriftProfile = drifter.create_drift_profile(
        arrow_table, psi_drift_config_with_categorical_features
    )
    drift_map = drifter.compute_drift(arrow_table, profile)

    assert len(drift_map.features) == 6


def test_psi_drift_normal_threshold(
    pandas_dataframe: pd.DataFrame,
    psi_drift_normal_threshold_psi_config: PsiDriftConfig,
):
    scouter = Drifter()
    scouter.create_drift_profile(
        pandas_dataframe,
        psi_drift_normal_threshold_psi_config,
    )


def test_psi_drift_chi_threshold(
    pandas_dataframe: pd.DataFrame,
    psi_drift_chi_threshold_psi_config: PsiDriftConfig,
):
    scouter = Drifter()
    scouter.create_drift_profile(
        pandas_dataframe,
        psi_drift_chi_threshold_psi_config,
    )


def test_update_psi_drift_config(
    pandas_categorical_dataframe: pd.DataFrame,
    psi_drift_config: PsiDriftConfig,
):
    drifter = Drifter()

    profile: PsiDriftProfile = drifter.create_drift_profile(
        pandas_categorical_dataframe,
        psi_drift_config,
    )

    original_space = profile.config.space
    original_name = profile.config.name
    original_binning_strategy = profile.config.binning_strategy

    profile.update_config_args(
        space="some_other_space",
        name="some_other_name",
        binning_strategy=EqualWidthBinning(method=SquareRoot()),
    )
    assert profile.config.space != original_space
    assert profile.config.name != original_name
    assert profile.config.binning_strategy != original_binning_strategy
