# type: ignore
from scouter import Profiler
from numpy.typing import NDArray
import polars as pl
from scouter import DataProfile
import pandas as pd


def test_multi_data_profile_with_correlation(
    polars_dataframe_multi_dtype_drift: pl.DataFrame,
):
    scouter = Profiler()
    profile: DataProfile = scouter.create_data_profile(
        polars_dataframe_multi_dtype_drift, compute_correlations=True
    )
    assert profile.correlations is not None

    corr_keys = list(profile.correlations.keys())
    corr_keys.sort()

    assert corr_keys == ["cat1", "cat2", "num1", "num2", "num3"]


def test_num_data_with_correlation(
    array: NDArray,
):
    scouter = Profiler()
    profile: DataProfile = scouter.create_data_profile(array, compute_correlations=True)
    assert profile.correlations is not None
    corr_keys = list(profile.correlations.keys())
    corr_keys.sort()

    assert corr_keys == ["feature_0", "feature_1", "feature_2"]


def test_string_data_with_correlation(
    pandas_categorical_dataframe: pd.DataFrame,
):
    scouter = Profiler()
    profile: DataProfile = scouter.create_data_profile(
        pandas_categorical_dataframe, compute_correlations=True
    )
    assert profile.correlations is not None
    corr_keys = list(profile.correlations.keys())
    corr_keys.sort()

    assert corr_keys == ["cat1", "cat2", "cat3"]
