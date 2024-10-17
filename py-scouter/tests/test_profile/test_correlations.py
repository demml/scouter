# type: ignore
from scouter import Profiler

import polars as pl
import pandas as pd
from numpy.typing import NDArray
import pytest
from scouter import DataProfile
from pathlib import Path


def test_data_profile_f64(array: NDArray):
    scouter = Profiler()
    profile: DataProfile = scouter.create_data_profile(array)
