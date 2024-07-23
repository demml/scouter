from scouter import Drifter
import numpy as np
from pathlib import Path
from numpy.typing import NDArray
import pytest
from scouter._scouter import (
    DriftProfile,
    DriftMap,
    DriftConfig,
    AlertRule,
)


def test_drift_f64(array: NDArray, monitor_config: DriftConfig):
    scouter = Drifter()
    profile: DriftProfile = scouter.create_drift_profile(array, monitor_config)

    # assert features are relatively centered
    assert profile.features["feature_0"].center == pytest.approx(1.5, 0.1)
    assert profile.features["feature_1"].center == pytest.approx(2.5, 0.1)
    assert profile.features["feature_2"].center == pytest.approx(3.5, 0.1)

    features = ["feature_0", "feature_1", "feature_2"]
    _ = scouter.compute_drift(array, profile, features)


def test_drift_f32(array: NDArray, monitor_config: DriftConfig):
    array = array.astype("float32")
    scouter = Drifter()
    profile: DriftProfile = scouter.create_drift_profile(array, monitor_config)

    # assert features are relatively centered
    assert profile.features["feature_0"].center == pytest.approx(1.5, 0.1)
    assert profile.features["feature_1"].center == pytest.approx(2.5, 0.1)
    assert profile.features["feature_2"].center == pytest.approx(3.5, 0.1)

    features = ["feature_0", "feature_1", "feature_2"]
    _ = scouter.compute_drift(array, profile, features)


def test_drift_int(array: NDArray, monitor_config: DriftConfig):
    # convert to int32
    array = array.astype("int32")

    scouter = Drifter()
    profile: DriftProfile = scouter.create_drift_profile(array, monitor_config)

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


def test_drift_fail(array: NDArray, monitor_config: DriftConfig):
    scouter = Drifter()
    profile: DriftProfile = scouter.create_drift_profile(array, monitor_config)
    features = ["feature_0", "feature_1", "feature_2"]

    with pytest.raises(ValueError):
        scouter.compute_drift(array.astype("str"), profile, features)


def test_alerts_control(array: NDArray, monitor_config: DriftConfig):
    scouter = Drifter()
    profile: DriftProfile = scouter.create_drift_profile(array, monitor_config)

    # assert features are relatively centered
    assert profile.features["feature_0"].center == pytest.approx(1.5, 0.1)
    assert profile.features["feature_1"].center == pytest.approx(2.5, 0.1)
    assert profile.features["feature_2"].center == pytest.approx(3.5, 0.1)

    features = ["feature_0", "feature_1", "feature_2"]
    drift_map: DriftMap = scouter.compute_drift(array, profile, features)

    # create drift array and features
    feature0 = drift_map.features["feature_0"]
    feature1 = drift_map.features["feature_1"]
    feature2 = drift_map.features["feature_2"]
    num_samples = len(feature0.drift)

    drift_array = np.zeros((num_samples, 3))

    # insert into drift array
    drift_array[:, 0] = feature0.drift
    drift_array[:, 1] = feature1.drift
    drift_array[:, 2] = feature2.drift

    # generate alerts

    alerts = scouter.generate_alerts(drift_array, features, AlertRule())

    # should have no alerts
    for feature in features:
        alert = alerts.features[feature]
        assert len(alert.alerts) <= 1

    array, features = drift_map.to_numpy()

    assert isinstance(array, np.ndarray)
    assert isinstance(features, list)


def test_alerts_percentage(array: NDArray, monitor_config_percentage: DriftConfig):
    scouter = Drifter()

    profile: DriftProfile = scouter.create_drift_profile(
        array, monitor_config_percentage
    )

    # assert features are relatively centered
    assert profile.features["feature_0"].center == pytest.approx(1.5, 0.1)
    assert profile.features["feature_1"].center == pytest.approx(2.5, 0.1)
    assert profile.features["feature_2"].center == pytest.approx(3.5, 0.1)

    features = ["feature_0", "feature_1", "feature_2"]

    drift_array: NDArray = np.asarray(
        [
            [0, 0, 0],
            [0, 0, 0],
            [0, 0, 0],
            [0, 0, 0],
            [0, 0, 0],
            [0, 0, 0],
            [0, 0, 0],
            [0, 0, 0],
            [0, 0, 0],
        ],
        dtype=np.float64,
    )

    # add drift
    drift_array[0, 0] = 1.0
    drift_array[8, 0] = 1.0

    alerts = scouter.generate_alerts(
        drift_array,
        features,
        monitor_config_percentage.alert_config.alert_rule,
    )

    # should have no alerts
    assert len(alerts.features["feature_0"].alerts) == 1
    assert len(alerts.features["feature_0"].indices[1]) == 2
