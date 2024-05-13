import pytest
import polars as pl
import numpy as np
from numpy.typing import NDArray


@pytest.fixture(scope="function")
def array() -> NDArray:
    array = np.random.rand(1000, 3)
    # add 1 to first column
    array[:, 0] += 1
    # add 2 to second column
    array[:, 1] += 2
    # add 3 to third column
    array[:, 2] += 3

    return array
