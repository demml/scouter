from scouter import Drifter

import polars as pl
import pandas as pd
from numpy.typing import NDArray
import pytest

from scouter._scouter import DriftProfile, DriftConfig


def test_monitor_f64(array: NDArray, monitor_config: DriftConfig):
    scouter = Drifter()
    profile: DriftProfile = scouter.create_drift_profile(array, monitor_config)

    # assert features are relatively centered
    assert profile.features["feature_0"].center == pytest.approx(1.5, 0.1)
    assert profile.features["feature_1"].center == pytest.approx(2.5, 0.1)
    assert profile.features["feature_2"].center == pytest.approx(3.5, 0.1)


def test_monitor_f32(array: NDArray, monitor_config: DriftConfig):
    # convert to float32
    array = array.astype("float32")

    scouter = Drifter()
    profile: DriftProfile = scouter.create_drift_profile(array, monitor_config)

    # assert features are relatively centered
    assert profile.features["feature_0"].center == pytest.approx(1.5, 0.1)
    assert profile.features["feature_1"].center == pytest.approx(2.5, 0.1)
    assert profile.features["feature_2"].center == pytest.approx(3.5, 0.1)


def test_monitor_polars(array: NDArray, monitor_config: DriftConfig):
    df = pl.from_numpy(array)
    scouter = Drifter()
    profile: DriftProfile = scouter.create_drift_profile(df, monitor_config)

    # assert features are relatively centered
    assert profile.features["column_0"].center == pytest.approx(1.5, 0.1)
    assert profile.features["column_1"].center == pytest.approx(2.5, 0.1)
    assert profile.features["column_2"].center == pytest.approx(3.5, 0.1)


def test_monitor_pandas(array: NDArray, monitor_config: DriftConfig):
    df = pd.DataFrame(array)
    scouter = Drifter()
    profile: DriftProfile = scouter.create_drift_profile(df, monitor_config)

    # assert features are relatively centered
    assert profile.features["0"].center == pytest.approx(1.5, 0.1)
    assert profile.features["1"].center == pytest.approx(2.5, 0.1)
    assert profile.features["2"].center == pytest.approx(3.5, 0.1)


def test_fail(array: NDArray, monitor_config: DriftConfig):
    scouter = Drifter()

    with pytest.raises(ValueError):
        scouter.create_drift_profile(array.astype("str"), monitor_config=monitor_config)


def test_int(array: NDArray, monitor_config: DriftConfig):
    # convert to int32
    array = array.astype("int32")

    scouter = Drifter()
    profile: DriftProfile = scouter.create_drift_profile(array, monitor_config)

    # assert features are relatively centered
    assert profile.features["feature_0"].center == pytest.approx(1.0, 0.1)
    assert profile.features["feature_1"].center == pytest.approx(2.0, 0.1)
    assert profile.features["feature_2"].center == pytest.approx(3.0, 0.1)
