from typing import Union
from numpy.typing import NDArray
import polars as pl
import pandas as pd

from ._scouter import create_data_profile  # pylint: disable=no-name-in-module


class Profiler:
    def __init__(self):
        """
        Class used to generate a data profile from a pandas dataframe,
        polars dataframe or numpy array.

        Args:
            feature_names:
                List of feature names.
            num_bins:
                Number of bins to use for the histogram.
        """

    def _convert_data_to_array(
        self, data: Union[pl.DataFrame, pd.DataFrame, NDArray]
    ) -> NDArray:
        if isinstance(data, pl.DataFrame):
            return data.to_numpy()
        if isinstance(data, pd.DataFrame):
            return data.to_numpy()
        return data

    def create_data_profile(
        self,
        data: Union[pl.DataFrame, pd.DataFrame, NDArray],
    ) -> None:
        array = self._convert_data_to_array(data)

        return create_data_profile(array=array)
