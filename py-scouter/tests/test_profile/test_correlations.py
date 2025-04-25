# type: ignore
import pandas as pd
import polars as pl
from numpy.typing import NDArray
from scouter import DataProfile, DataProfiler


def test_multi_data_profile_with_correlation(
    polars_dataframe_multi_dtype_drift: pl.DataFrame,
):
    scouter = DataProfiler()
    profile: DataProfile = scouter.create_data_profile(polars_dataframe_multi_dtype_drift, compute_correlations=True)
    for features in profile.features.values():
        assert features.correlations is not None


def test_num_data_with_correlation(
    array: NDArray,
):
    scouter = DataProfiler()
    profile: DataProfile = scouter.create_data_profile(array, compute_correlations=True)
    for features in profile.features.values():
        assert features.correlations is not None


def test_string_data_with_correlation(
    pandas_categorical_dataframe: pd.DataFrame,
):
    scouter = DataProfiler()
    profile: DataProfile = scouter.create_data_profile(pandas_categorical_dataframe, compute_correlations=True)

    for features in profile.features.values():
        assert features.correlations is not None
