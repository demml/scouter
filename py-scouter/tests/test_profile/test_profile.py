from scouter import Scouter

import polars as pl
import pandas as pd
from numpy.typing import NDArray
import pytest
from scouter import DataProfile
from pathlib import Path


def test_data_profile_f64(array: NDArray):
    scouter = Scouter()
    profile: DataProfile = scouter.create_data_profile(array)

    # assert features are relatively centered
    assert profile.features["feature_0"].mean == pytest.approx(1.5, 0.1)
    assert profile.features["feature_0"].distinct.count == pytest.approx(1000, 1)
    assert profile.features["feature_0"].quantiles.q25 == pytest.approx(1.25, 0.1)
    assert profile.features["feature_0"].histogram.bins[0] == pytest.approx(1.00, 0.1)
    assert len(profile.features["feature_0"].histogram.bin_counts) == 20

    # convert to json
    json_str = profile.model_dump_json()

    # load from json
    loaded_profile = DataProfile.load_from_json(json_str)

    assert loaded_profile.features["feature_0"].mean == pytest.approx(1.5, 0.1)

    # save to json
    loaded_profile.save_to_json(Path("assets/data_profile.json"))
    assert Path("assets/data_profile.json").exists()

    # save to different path, should be converted to json
    loaded_profile.save_to_json(Path("assets/data1_profile.joblib"))
    assert Path("assets/data1_profile.json").exists()

    loaded_profile.save_to_json()

    assert Path("data_profile.json").exists()
    Path("data_profile.json").unlink()


def test_data_profile_f32(array: NDArray):
    array = array.astype("float32")
    scouter = Scouter()
    profile: DataProfile = scouter.create_data_profile(array)

    # assert features are relatively centered
    assert profile.features["feature_0"].mean == pytest.approx(1.5, 0.1)
    assert profile.features["feature_0"].distinct.count == pytest.approx(1000, 1)
    assert profile.features["feature_0"].quantiles.q25 == pytest.approx(1.25, 0.1)
    assert profile.features["feature_0"].histogram.bins[0] == pytest.approx(1.00, 0.1)
    assert len(profile.features["feature_0"].histogram.bin_counts) == 20


def test_data_profile_polars(array: NDArray):
    df = pl.from_numpy(array)
    scouter = Scouter()
    profile: DataProfile = scouter.create_data_profile(df)

    # assert features are relatively centered
    assert profile.features["column_0"].mean == pytest.approx(1.5, 0.1)
    assert profile.features["column_0"].distinct.count == pytest.approx(1000, 1)
    assert profile.features["column_0"].quantiles.q25 == pytest.approx(1.25, 0.1)
    assert profile.features["column_0"].histogram.bins[0] == pytest.approx(1.00, 0.1)
    assert len(profile.features["column_0"].histogram.bin_counts) == 20


def test_data_profile_pandas(array: NDArray):
    df = pd.DataFrame(array)
    scouter = Scouter()
    profile: DataProfile = scouter.create_data_profile(df)

    # assert features are relatively centered
    assert profile.features["0"].mean == pytest.approx(1.5, 0.1)
    assert profile.features["0"].distinct.count == pytest.approx(1000, 1)
    assert profile.features["0"].quantiles.q25 == pytest.approx(1.25, 0.1)
    assert profile.features["0"].histogram.bins[0] == pytest.approx(1.00, 0.1)
    assert len(profile.features["0"].histogram.bin_counts) == 20
