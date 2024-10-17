# type: ignore
from scouter import Profiler

import polars as pl
import pandas as pd
from numpy.typing import NDArray
import pytest
from scouter import DataProfile
from pathlib import Path


def test_data_profile_f64(polars_dataframe_multi_dtype_drift: pl.DataFrame):
    scouter = Profiler()
    profile: DataProfile = scouter.create_data_profile(
        polars_dataframe_multi_dtype_drift, compute_correlations=True
    )
    assert profile.correlations is not None
    print(profile.correlations.keys().sort())
    a
