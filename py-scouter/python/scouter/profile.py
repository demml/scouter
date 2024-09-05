# pylint: disable=pointless-statement,broad-exception-caught


from typing import Union

import pandas as pd
import polars as pl
import pyarrow as pa  # type: ignore
from numpy.typing import NDArray
from scouter.utils.logger import ScouterLogger
from scouter.utils.type_converter import _convert_data_to_array, _get_bits

from ._scouter import DataProfile, ScouterProfiler  # pylint: disable=no-name-in-module

logger = ScouterLogger.get_logger()


class Profiler:
    def __init__(self) -> None:
        """Scouter class for creating data profiles. This class will generate
        baseline statistics for a given dataset."""
        self._profiler = ScouterProfiler()

    def create_data_profile(
        self,
        data: Union[pl.DataFrame, pd.DataFrame, NDArray, pa.Table],
        bin_size: int = 20,
    ) -> DataProfile:
        """Create a data profile from data.

        Args:
            data:
                Data to create a data profile from. Data can be a numpy array,
                a polars dataframe or pandas dataframe. Data is expected to not contain
                any missing values, NaNs or infinities. These values must be removed or imputed.
                If NaNs or infinities are present, the data profile will not be created.
            bin_size:
                Optional bin size for histograms. Defaults to 20 bins.

        Returns:
            Monitoring profile
        """
        try:
            logger.info("Creating data profile.")

            array = _convert_data_to_array(data)
            bits = _get_bits(array.numeric_array)

            profile = getattr(self._profiler, f"create_data_profile_f{bits}")(
                numeric_array=array.numeric_array,
                string_array=array.string_array,
                numeric_features=array.numeric_features,
                string_features=array.string_features,
                bin_size=bin_size,
            )

            assert isinstance(profile, DataProfile), f"Expected DataProfile, got {type(profile)}"
            return profile

        except Exception as exc:  # type: ignore
            logger.error(f"Failed to create data profile: {exc}")
            raise ValueError(f"Failed to create data profile: {exc}") from exc
