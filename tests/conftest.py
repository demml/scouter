import pytest
import polars as pl
import numpy as np


@pytest.fixture(scope="module")
def test_polars_dataframe():
    df = pl.DataFrame(
        {
            "int": [1, 2, 3, 4, 5, 6],
            "str": ["a", "b", "c", "d", "e", "f"],
            "float": [1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
        }
    )
    return df


@pytest.fixture(scope="module")
def test_pandas_dataframe(test_polars_dataframe):
    return test_polars_dataframe.to_pandas()


@pytest.fixture(scope="module")
def test_numpy_array():
    return np.random.rand(1_000, 3)
