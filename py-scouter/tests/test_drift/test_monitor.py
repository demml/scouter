import pytest
from numpy.typing import NDArray
from scouter import (  # type: ignore[attr-defined]
    Drifter,
    SpcDriftConfig,
    SpcDriftProfile,
)


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


def test_monitor_polars(polars_dataframe, drift_config: SpcDriftConfig):
    scouter = Drifter()
    profile: SpcDriftProfile = scouter.create_drift_profile(polars_dataframe, drift_config)

    # assert features are relatively centered
    assert profile.features["feature_0"].center == pytest.approx(1.5, 0.1)
    assert profile.features["feature_1"].center == pytest.approx(2.5, 0.1)
    assert profile.features["feature_2"].center == pytest.approx(3.5, 0.1)


def test_monitor_pandas(pandas_dataframe, drift_config: SpcDriftConfig):
    scouter = Drifter()
    profile: SpcDriftProfile = scouter.create_drift_profile(pandas_dataframe, drift_config)

    # assert features are relatively centered
    assert profile.features["feature_0"].center == pytest.approx(1.5, 0.1)
    assert profile.features["feature_1"].center == pytest.approx(2.5, 0.1)
    assert profile.features["feature_2"].center == pytest.approx(3.5, 0.1)


def test_fail(array: NDArray, drift_config: SpcDriftConfig):
    scouter = Drifter()

    with pytest.raises(RuntimeError):
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
