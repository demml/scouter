# type: ignore
from pathlib import Path

import pandas as pd
import polars as pl
import pytest
from numpy.typing import NDArray
from scouter import DataProfile, DataProfiler
from scouter.types import ScouterError


def test_data_profile_f64(array: NDArray):
    scouter = DataProfiler()
    profile: DataProfile = scouter.create_data_profile(array)

    # assert features are relatively centered
    assert profile.features["feature_0"].numeric_stats.mean == pytest.approx(1.5, 0.1)
    assert profile.features["feature_0"].numeric_stats.distinct.count == pytest.approx(
        1000, 1
    )
    assert profile.features["feature_0"].numeric_stats.quantiles.q25 == pytest.approx(
        1.25, 0.1
    )
    assert profile.features["feature_0"].numeric_stats.histogram.bins[
        0
    ] == pytest.approx(1.00, 0.1)
    assert len(profile.features["feature_0"].numeric_stats.histogram.bin_counts) == 20

    # convert to json
    json_str = profile.model_dump_json()

    # load from json
    loaded_profile = DataProfile.model_validate_json(json_str)

    assert loaded_profile.features["feature_0"].numeric_stats.mean == pytest.approx(
        1.5, 0.1
    )

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
    scouter = DataProfiler()
    profile: DataProfile = scouter.create_data_profile(array)

    # assert features are relatively centered
    assert profile.features["feature_0"].numeric_stats.mean == pytest.approx(1.5, 0.1)
    assert profile.features["feature_0"].numeric_stats.distinct.count == pytest.approx(
        1000, 1
    )
    assert profile.features["feature_0"].numeric_stats.quantiles.q25 == pytest.approx(
        1.25, 0.1
    )
    assert profile.features["feature_0"].numeric_stats.histogram.bins[
        0
    ] == pytest.approx(1.00, 0.1)
    assert len(profile.features["feature_0"].numeric_stats.histogram.bin_counts) == 20


def test_data_profile_polars(array: NDArray):
    df = pl.from_numpy(array)
    scouter = DataProfiler()
    profile: DataProfile = scouter.create_data_profile(df)

    # assert features are relatively centered
    assert profile.features["column_0"].numeric_stats.mean == pytest.approx(1.5, 0.1)
    assert profile.features["column_0"].numeric_stats.distinct.count == pytest.approx(
        1000, 1
    )
    assert profile.features["column_0"].numeric_stats.quantiles.q25 == pytest.approx(
        1.25, 0.1
    )
    assert profile.features["column_0"].numeric_stats.histogram.bins[
        0
    ] == pytest.approx(1.00, 0.1)
    assert len(profile.features["column_0"].numeric_stats.histogram.bin_counts) == 20


def test_data_profile_pandas(array: NDArray):
    df = pd.DataFrame(array)
    scouter = DataProfiler()

    with pytest.raises(ScouterError) as error:
        profile: DataProfile = scouter.create_data_profile(df)

    assert str(error.value) == "Column names must be string type"

    df.columns = df.columns.astype(str)
    profile: DataProfile = scouter.create_data_profile(df)

    # assert features are relatively centered
    assert profile.features["0"].numeric_stats.mean == pytest.approx(1.5, 0.1)
    assert profile.features["0"].numeric_stats.distinct.count == pytest.approx(1000, 1)
    assert profile.features["0"].numeric_stats.quantiles.q25 == pytest.approx(1.25, 0.1)
    assert profile.features["0"].numeric_stats.histogram.bins[0] == pytest.approx(
        1.00, 0.1
    )
    assert len(profile.features["0"].numeric_stats.histogram.bin_counts) == 20


def test_data_profile_polars_mixed_type(
    polars_dataframe_multi_dtype: pl.DataFrame,
):
    scouter = DataProfiler()
    profile: DataProfile = scouter.create_data_profile(polars_dataframe_multi_dtype)

    assert profile.features["cat2"].string_stats.distinct.count == 3
    assert profile.features["cat2"].string_stats.word_stats.words[
        "3.0"
    ].percent == pytest.approx(0.352, abs=0.1)

    assert profile.features["cat1"].string_stats.distinct.count == 5
    assert profile.features["cat1"].string_stats.word_stats.words[
        "7.0"
    ].percent == pytest.approx(0.19, abs=0.1)


def test_data_profile_pandas_mixed_type(
    polars_dataframe_multi_dtype: pl.DataFrame,
):
    scouter = DataProfiler()
    profile: DataProfile = scouter.create_data_profile(
        polars_dataframe_multi_dtype.to_pandas()
    )

    assert profile.features["cat2"].string_stats.distinct.count == 3
    assert profile.features["cat2"].string_stats.word_stats.words[
        "3.0"
    ].percent == pytest.approx(0.352, abs=0.1)

    assert profile.features["cat1"].string_stats.distinct.count == 5
    assert profile.features["cat1"].string_stats.word_stats.words[
        "7.0"
    ].percent == pytest.approx(0.19, abs=0.1)


def test_data_profile_pyarrow_mixed_type(
    polars_dataframe_multi_dtype: pl.DataFrame,
):
    arrow_table = polars_dataframe_multi_dtype.to_arrow()

    scouter = DataProfiler()
    profile: DataProfile = scouter.create_data_profile(arrow_table)

    assert profile.features["cat2"].string_stats.distinct.count == 3
    assert profile.features["cat2"].string_stats.word_stats.words[
        "3.0"
    ].percent == pytest.approx(0.352, abs=0.1)

    assert profile.features["cat1"].string_stats.distinct.count == 5
    assert profile.features["cat1"].string_stats.word_stats.words[
        "7.0"
    ].percent == pytest.approx(0.19, abs=0.1)
