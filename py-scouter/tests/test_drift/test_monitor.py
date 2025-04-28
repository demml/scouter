import pandas as pd
import polars as pl
import pytest
from numpy.typing import NDArray
from scouter import (  # type: ignore[attr-defined]
    Drifter,
    SpcDriftConfig,
    SpcDriftProfile,
)
from scouter.types import ScouterError


def test_monitor_f64(array: NDArray, drift_config: SpcDriftConfig):
    scouter = Drifter()
    profile: SpcDriftProfile = scouter.create_drift_profile(array, drift_config)

    # assert features are relatively centered
    assert profile.features["feature_0"].center == pytest.approx(1.5, 0.1)
    assert profile.features["feature_1"].center == pytest.approx(2.5, 0.1)
    assert profile.features["feature_2"].center == pytest.approx(3.5, 0.1)


def test_monitor_f32(array: NDArray, drift_config: SpcDriftConfig):
    # convert to float32
    array = array.astype("float32")

    scouter = Drifter()
    profile: SpcDriftProfile = scouter.create_drift_profile(array, drift_config)

    # assert features are relatively centered
    assert profile.features["feature_0"].center == pytest.approx(1.5, 0.1)
    assert profile.features["feature_1"].center == pytest.approx(2.5, 0.1)
    assert profile.features["feature_2"].center == pytest.approx(3.5, 0.1)


def test_monitor_polars(array: NDArray, drift_config: SpcDriftConfig):
    df = pl.from_numpy(array)
    scouter = Drifter()
    profile: SpcDriftProfile = scouter.create_drift_profile(df, drift_config)

    # assert features are relatively centered
    assert profile.features["column_0"].center == pytest.approx(1.5, 0.1)
    assert profile.features["column_1"].center == pytest.approx(2.5, 0.1)
    assert profile.features["column_2"].center == pytest.approx(3.5, 0.1)


def test_monitor_pandas(array: NDArray, drift_config: SpcDriftConfig):
    df = pd.DataFrame(array)
    # convert column names to string
    df.columns = [str(col) for col in df.columns]  # type: ignore
    scouter = Drifter()
    profile: SpcDriftProfile = scouter.create_drift_profile(df, drift_config)

    # assert features are relatively centered
    assert profile.features["0"].center == pytest.approx(1.5, 0.1)
    assert profile.features["1"].center == pytest.approx(2.5, 0.1)
    assert profile.features["2"].center == pytest.approx(3.5, 0.1)


def test_fail(array: NDArray, drift_config: SpcDriftConfig):
    scouter = Drifter()

    with pytest.raises(ScouterError):
        scouter.create_drift_profile(array.astype("str"), config=drift_config)


def test_int(array: NDArray, drift_config: SpcDriftConfig):
    # convert to int32
    array = array.astype("int32")

    scouter = Drifter()
    profile: SpcDriftProfile = scouter.create_drift_profile(array, drift_config)

    # assert features are relatively centered
    assert profile.features["feature_0"].center == pytest.approx(1.0, 0.1)
    assert profile.features["feature_1"].center == pytest.approx(2.0, 0.1)
    assert profile.features["feature_2"].center == pytest.approx(3.0, 0.1)
