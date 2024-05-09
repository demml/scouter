from typing import Union
from numpy.typing import NDArray, Optional, List
import polars as pl
import pandas as pd
from scouter.utils.logger import ScouterLogger

from ._scouter import RustScouter  # pylint: disable=no-name-in-module

logger = ScouterLogger.get_logger()


class Scouter:
    def __init__(self, features: Optional[List[str]] = None) -> None:
        """
        Scouter generates data profiles and monitoring profiles from arrays. Accepted
        array types include numpy arrays, polars dataframes and pandas dataframes.


        Args:
            feature_names:
                Optional list of feature names.
        """
        self._scouter = RustScouter(features)

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

        return self._profiler.create_data_profile(array=array)

    def create_monitoring_profile(
        self,
        data: Union[pl.DataFrame, pd.DataFrame, NDArray],
    ) -> None:
        # convert data to numpy array
        array = self._convert_data_to_array(data)

        # get numpy array type
        dtype = array.dtype

        # check if numpy array is float32
        if dtype == "float32":
            return self._scouter.create_monitoring_profile32(array=array)

        if dtype == "float64":
            return self._scouter.create_monitoring_profile64(array=array)

        # if numpy array is integer, convert to float32
        if dtype in ["int8", "int16", "int32", "int64"]:
            logger.warning(
                "Scouter only supports float32 and float64 arrays. Converting integer array to float32."
            )
            array = array.astype("float32")
            return self._scouter.create_monitoring_profile32(array=array)

        return self._profiler.create_monitoring_profile(array=array)
