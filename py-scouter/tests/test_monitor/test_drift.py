from scouter import Scouter


from numpy.typing import NDArray
import pytest
from scouter._scouter import MonitorProfile, DriftMap, MonitorConfig
from pathlib import Path


def test_drift_f64(array: NDArray, monitor_config: MonitorConfig):
    scouter = Scouter()
    profile: MonitorProfile = scouter.create_monitoring_profile(array, monitor_config)

    # assert features are relatively centered
    assert profile.features["feature_0"].center == pytest.approx(1.5, 0.1)
    assert profile.features["feature_1"].center == pytest.approx(2.5, 0.1)
    assert profile.features["feature_2"].center == pytest.approx(3.5, 0.1)

    features = ["feature_0", "feature_1", "feature_2"]
    _ = scouter.compute_drift(array, profile, features)


def test_drift_f32(array: NDArray, monitor_config: MonitorConfig):
    array = array.astype("float32")
    scouter = Scouter()
    profile: MonitorProfile = scouter.create_monitoring_profile(array, monitor_config)

    # assert features are relatively centered
    assert profile.features["feature_0"].center == pytest.approx(1.5, 0.1)
    assert profile.features["feature_1"].center == pytest.approx(2.5, 0.1)
    assert profile.features["feature_2"].center == pytest.approx(3.5, 0.1)

    features = ["feature_0", "feature_1", "feature_2"]
    _ = scouter.compute_drift(array, profile, features)


def test_drift_int(array: NDArray, monitor_config: MonitorConfig):
    # convert to int32
    array = array.astype("int32")

    scouter = Scouter()
    profile: MonitorProfile = scouter.create_monitoring_profile(array, monitor_config)

    # assert features are relatively centered
    assert profile.features["feature_0"].center == pytest.approx(1.0, 0.1)
    assert profile.features["feature_1"].center == pytest.approx(2.0, 0.1)
    assert profile.features["feature_2"].center == pytest.approx(3.0, 0.1)

    features = ["feature_0", "feature_1", "feature_2"]
    drift_map = scouter.compute_drift(array, profile, features)

    assert drift_map.features["feature_0"].drift[0] == 0.0

    model = drift_map.model_dump_json()

    loaded_model = DriftMap.load_from_json(model)

    assert loaded_model.features["feature_0"].drift[0] == 0.0

    # save to json
    # saves to drift_map.json
    drift_map.save_to_json(Path("assets/drift_map.json"))

    # assert file exists
    assert Path("assets/drift_map.json").exists()

    # save to different path, should be converted to json
    drift_map.save_to_json(Path("assets/model.joblib"))

    assert Path("assets/model.json").exists()


def test_drift_fail(array: NDArray, monitor_config: MonitorConfig):
    scouter = Scouter()
    profile: MonitorProfile = scouter.create_monitoring_profile(array, monitor_config)
    features = ["feature_0", "feature_1", "feature_2"]

    with pytest.raises(ValueError):
        scouter.compute_drift(array.astype("str"), profile, features)
