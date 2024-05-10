from numpy.typing import NDArray
from concurrent.futures import ThreadPoolExecutor
import numpy as np
from typing import Dict, Union
import polars as pl
import pandas as pd


def _compute_c4(sample_size: int) -> float:
    left = 4.0 * sample_size - 4.0
    right = 4.0 * sample_size - 3.0

    return left / right


def _compute_stats(array: NDArray):
    return {
        "mean": array.mean(),
        "std": array.std(),
    }


def _create_monitoring_profile(array: NDArray) -> Dict[str, float]:
    samples = np.array_split(array, 100)

    mean = []
    sd = []
    for sample in samples:
        stats = _compute_stats(sample)
        mean.append(stats["mean"])
        sd.append(stats["std"])

    # compute ucl lcl and mean
    mean = np.mean(mean)
    sd = np.mean(sd)
    ucl = mean + 3 * sd
    lcl = mean - 3 * sd

    return {
        "mean": mean,
        "ucl": ucl,
        "lcl": lcl,
    }


# hacky code use for testing
class NumpyScouter:
    def __init__(self):
        self.features = "hello"

    def _convert_data_to_array(
        self, data: Union[pl.DataFrame, pd.DataFrame, NDArray]
    ) -> NDArray:
        if isinstance(data, pl.DataFrame):
            return data.to_numpy()
        if isinstance(data, pd.DataFrame):
            return data.to_numpy()
        return data

    def create_monitoring_profile(self, array: NDArray):
        executor = ThreadPoolExecutor()
        feats = array.shape[1]
        rows = array.shape[0]

        array = self._convert_data_to_array(array)

        # mock feature checking
        if self.features == "hello":
            pass

        if len(array.shape) != 2:
            raise ValueError("Array must be 2D")

        # mock sample checking
        if rows < 100:
            sample_size = 25
        else:
            sample_size = 100

        _ = _compute_c4(sample_size)

        # create a monitoring profile for each feature
        with ThreadPoolExecutor() as executor:
            results = list(
                executor.map(
                    _create_monitoring_profile, [array[:, i] for i in range(feats)]
                )
            )


if __name__ == "__main__":
    import numpy as np

    array = np.random.rand(1_000_000, 100)
    NumpyScouter().create_monitoring_profile(array)
