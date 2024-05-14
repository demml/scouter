from scouter import Scouter

import polars as pl
import pandas as pd
from numpy.typing import NDArray
import pytest
from scouter._scouter import MonitorProfile
from scouter import DataProfile


def test_monitor_f64(array: NDArray):
    scouter = Scouter()
    profile: MonitorProfile = scouter.create_monitoring_profile(array)

    # assert features are relatively centered
    assert profile.features["feature_0"].center == pytest.approx(1.5, 0.1)
    assert profile.features["feature_1"].center == pytest.approx(2.5, 0.1)
    assert profile.features["feature_2"].center == pytest.approx(3.5, 0.1)


def test_monitor_f32(array: NDArray):
    # convert to float32
    array = array.astype("float32")

    scouter = Scouter()
    profile: MonitorProfile = scouter.create_monitoring_profile(array)

    # assert features are relatively centered
    assert profile.features["feature_0"].center == pytest.approx(1.5, 0.1)
    assert profile.features["feature_1"].center == pytest.approx(2.5, 0.1)
    assert profile.features["feature_2"].center == pytest.approx(3.5, 0.1)


def test_monitor_polars(array: NDArray):
    df = pl.from_numpy(array)
    scouter = Scouter()
    profile: MonitorProfile = scouter.create_monitoring_profile(df)

    # assert features are relatively centered
    assert profile.features["column_0"].center == pytest.approx(1.5, 0.1)
    assert profile.features["column_1"].center == pytest.approx(2.5, 0.1)
    assert profile.features["column_2"].center == pytest.approx(3.5, 0.1)


def test_monitor_pandas(array: NDArray):
    df = pd.DataFrame(array)
    scouter = Scouter()
    profile: MonitorProfile = scouter.create_monitoring_profile(df)

    # assert features are relatively centered
    assert profile.features["0"].center == pytest.approx(1.5, 0.1)
    assert profile.features["1"].center == pytest.approx(2.5, 0.1)
    assert profile.features["2"].center == pytest.approx(3.5, 0.1)


def test_fail(array: NDArray):
    scouter = Scouter()
    with pytest.raises(ValueError):
        scouter.create_monitoring_profile(data="fail")

    with pytest.raises(ValueError):
        scouter.create_monitoring_profile(array.astype("str"))


def test_int(array: NDArray):
    # convert to int32
    array = array.astype("int32")

    scouter = Scouter()
    profile: MonitorProfile = scouter.create_monitoring_profile(array)

    # assert features are relatively centered
    assert profile.features["feature_0"].center == pytest.approx(1.0, 0.1)
    assert profile.features["feature_1"].center == pytest.approx(2.0, 0.1)
    assert profile.features["feature_2"].center == pytest.approx(3.0, 0.1)


def test_data_profile_f64(array: NDArray):
    scouter = Scouter()
    profile: DataProfile = scouter.create_data_profile(array)

    # assert features are relatively centered
    assert profile.features["feature_0"].mean == pytest.approx(1.5, 0.1)
    assert profile.features["feature_0"].distinct.count == 1000
    assert profile.features["feature_0"].quantiles.q25 == pytest.approx(1.25, 0.1)
    assert profile.features["feature_0"].histogram.bins[0] == pytest.approx(1.00, 0.1)
    assert len(profile.features["feature_0"].histogram.bin_counts) == 20


def test_data_profile_f32(array: NDArray):
    array = array.astype("float32")
    scouter = Scouter()
    profile: DataProfile = scouter.create_data_profile(array)

    # assert features are relatively centered
    assert profile.features["feature_0"].mean == pytest.approx(1.5, 0.1)
    assert profile.features["feature_0"].distinct.count == 1000
    assert profile.features["feature_0"].quantiles.q25 == pytest.approx(1.25, 0.1)
    assert profile.features["feature_0"].histogram.bins[0] == pytest.approx(1.00, 0.1)
    assert len(profile.features["feature_0"].histogram.bin_counts) == 20


def test_data_profile_polars(array: NDArray):
    df = pl.from_numpy(array)
    scouter = Scouter()
    profile: DataProfile = scouter.create_data_profile(df)

    # assert features are relatively centered
    assert profile.features["column_0"].mean == pytest.approx(1.5, 0.1)
    assert profile.features["column_0"].distinct.count == 1000
    assert profile.features["column_0"].quantiles.q25 == pytest.approx(1.25, 0.1)
    assert profile.features["column_0"].histogram.bins[0] == pytest.approx(1.00, 0.1)
    assert len(profile.features["column_0"].histogram.bin_counts) == 20


def test_data_profile_pandas(array: NDArray):
    df = pd.DataFrame(array)
    scouter = Scouter()
    profile: DataProfile = scouter.create_data_profile(df)

    # assert features are relatively centered
    assert profile.features["0"].mean == pytest.approx(1.5, 0.1)
    assert profile.features["0"].distinct.count == 1000
    assert profile.features["0"].quantiles.q25 == pytest.approx(1.25, 0.1)
    assert profile.features["0"].histogram.bins[0] == pytest.approx(1.00, 0.1)
    assert len(profile.features["0"].histogram.bin_counts) == 20
