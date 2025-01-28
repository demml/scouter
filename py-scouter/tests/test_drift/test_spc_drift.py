from pathlib import Path
from tempfile import TemporaryDirectory

import numpy as np
import pandas as pd
import polars as pl
import pytest
from numpy.typing import NDArray
from scouter.alert import AlertDispatchType, AlertZone, SpcAlertConfig
from scouter.drift import Drifter, SpcDriftConfig, SpcDriftMap, SpcDriftProfile


def test_drift_f64(array: NDArray, drift_config: SpcDriftConfig):
    drifter = Drifter()
    profile: SpcDriftProfile = drifter.create_drift_profile(array, drift_config)

    # assert features are relatively centered
    assert profile.features["feature_0"].center == pytest.approx(1.5, 0.1)
    assert profile.features["feature_1"].center == pytest.approx(2.5, 0.1)
    assert profile.features["feature_2"].center == pytest.approx(3.5, 0.1)

    # save profile to json
    with TemporaryDirectory() as temp_dir:
        path = Path(temp_dir) / "profile.json"
        profile.save_to_json(path)
        assert (Path(temp_dir) / "profile.json").exists()

        # test loading from json file
        with open(Path(temp_dir) / "profile.json", "r") as f:
            SpcDriftProfile.model_validate_json(f.read())
            
        SpcDriftProfile.from_file(path)

    _ = drifter.compute_drift(array, profile)

    profile.update_config_args(repository="repo1", name="name1")

    assert profile.config.name == "name1"
    assert profile.config.repository == "repo1"


def test_drift_f32(array: NDArray, drift_config: SpcDriftConfig):
    array = array.astype("float32")
    scouter = Drifter()
    profile: SpcDriftProfile = scouter.create_drift_profile(array, drift_config)

    # assert features are relatively centered
    assert profile.features["feature_0"].center == pytest.approx(1.5, 0.1)
    assert profile.features["feature_1"].center == pytest.approx(2.5, 0.1)
    assert profile.features["feature_2"].center == pytest.approx(3.5, 0.1)

    _ = scouter.compute_drift(array, profile)


def test_drift_int(array: NDArray, drift_config: SpcDriftConfig):
    # convert to int32
    array = array.astype("int32")

    scouter = Drifter()
    profile: SpcDriftProfile = scouter.create_drift_profile(array, drift_config)

    # assert features are relatively centered
    assert profile.features["feature_0"].center == pytest.approx(1.0, 0.1)
    assert profile.features["feature_1"].center == pytest.approx(2.0, 0.1)
    assert profile.features["feature_2"].center == pytest.approx(3.0, 0.1)

    drift_map: SpcDriftMap = scouter.compute_drift(array, profile)

    assert drift_map.features["feature_0"].drift[0] == 0.0

    model = drift_map.model_dump_json()

    loaded_model = SpcDriftMap.model_validate_json(model)

    assert loaded_model.features["feature_0"].drift[0] == 0.0

    # save to json
    # saves to drift_map.json
    drift_map.save_to_json(Path("assets/drift_map.json"))

    # assert file exists
    assert Path("assets/drift_map.json").exists()

    # save to different path, should be converted to json
    drift_map.save_to_json(Path("assets/model.joblib"))

    assert Path("assets/model.json").exists()


def test_alerts_control(array: NDArray, drift_config: SpcDriftConfig):
    scouter = Drifter()
    profile: SpcDriftProfile = scouter.create_drift_profile(array, drift_config)

    # assert features are relatively centered
    assert profile.features["feature_0"].center == pytest.approx(1.5, 0.1)
    assert profile.features["feature_1"].center == pytest.approx(2.5, 0.1)
    assert profile.features["feature_2"].center == pytest.approx(3.5, 0.1)

    drift_map: SpcDriftMap = scouter.compute_drift(array, profile)

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


def test_multi_type_drift(
    polars_dataframe_multi_dtype: pl.DataFrame,
    polars_dataframe_multi_dtype_drift: pl.DataFrame,
    drift_config: SpcDriftConfig,
):
    drifter = Drifter()

    profile: SpcDriftProfile = drifter.create_drift_profile(polars_dataframe_multi_dtype, drift_config)

    drift_map = drifter.compute_drift(polars_dataframe_multi_dtype_drift, profile)
    assert len(drift_map.features) == 5

    drift_map.to_numpy()


def test_only_string_drift(pandas_categorical_dataframe: pd.DataFrame, drift_config: SpcDriftConfig):
    drifter = Drifter()

    profile: SpcDriftProfile = drifter.create_drift_profile(pandas_categorical_dataframe, drift_config)

    drift_map = drifter.compute_drift(pandas_categorical_dataframe, profile)

    assert len(drift_map.features) == 3


def test_data_pyarrow_mixed_type(
    polars_dataframe_multi_dtype: pl.DataFrame,
    drift_config: SpcDriftConfig,
):
    arrow_table = polars_dataframe_multi_dtype.to_arrow()

    drifter = Drifter()

    profile: SpcDriftProfile = drifter.create_drift_profile(arrow_table, drift_config)
    drift_map = drifter.compute_drift(arrow_table, profile)

    assert len(drift_map.features) == 5


def test_drift_config_alert_kwargs():
    alert_config = SpcAlertConfig(
        dispatch_kwargs={"channel": "scouter"},
        dispatch_type=AlertDispatchType.Slack,
    )
    config = SpcDriftConfig(
        name="test",
        repository="test",
        alert_config=alert_config,
    )

    assert config.alert_config.rule.zones_to_monitor == [
        AlertZone.Zone1,
        AlertZone.Zone2,
        AlertZone.Zone3,
        AlertZone.Zone4,
    ]

    assert config.alert_config.dispatch_kwargs["channel"] == "scouter"
    assert config.alert_config.dispatch_type == AlertDispatchType.Slack.value


def test_load_from_file():
    config = SpcDriftConfig(config_path="tests/assets/drift_config.json")
    assert config.name == "name"
    assert config.repository == "repo"


def test_load_from_file_error():
    with pytest.raises(RuntimeError) as e:
        SpcDriftConfig(config_path="tests/assets/drift_config_error.json")

    assert "Failed to deserialize string" in str(e)


def test_empty_params():
    config = SpcDriftConfig()

    assert config.name == "__missing__"
    assert config.repository == "__missing__"

    # update
    config.name = "name"
    config.repository = "repo"

    assert config.name == "name"
    assert config.repository == "repo"

    config.update_config_args(name="name1", repository="repo1")

    assert config.name == "name1"
    assert config.repository == "repo1"
