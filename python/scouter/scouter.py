from typing import Union, Optional, List, Dict
from numpy.typing import NDArray
from pydantic import BaseModel

# import polars as pl
import pandas as pd
from scouter.utils.logger import ScouterLogger

from ._scouter import RustScouter  # pylint: disable=no-name-in-module

logger = ScouterLogger.get_logger()


class FeatureMonitorProfile(BaseModel):
    id: str
    center: float
    lcl: float
    ucl: float
    timestamp: str


class MonitoringProfile(BaseModel):
    features: Dict[str, FeatureMonitorProfile]


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

    def _convert_data_to_array(self, data: Union[pd.DataFrame, NDArray]) -> NDArray:
        # if isinstance(data, pl.DataFrame):
        # return data.to_numpy()
        if isinstance(data, pd.DataFrame):
            return data.to_numpy()
        return data

    def create_data_profile(self, data: Union[pd.DataFrame, NDArray]) -> None:
        array = self._convert_data_to_array(data)

        return self._profiler.create_data_profile(array=array)

    def _get_monitoring_profile(self, data: Union[pd.DataFrame, NDArray]) -> str:
        # convert data to numpy array
        array = self._convert_data_to_array(data)

        # get numpy array type
        dtype = array.dtype

        # check if numpy array is float32
        if dtype == "float32":
            return self._scouter.create_monitor_profile_f32(array=array)

        if dtype == "float64":
            return self._scouter.create_monitor_profile_f64(array=array)

        # if numpy array is integer, convert to float32
        if dtype in ["int8", "int16", "int32", "int64"]:
            logger.warning(
                "Scouter only supports float32 and float64 arrays. Converting integer array to float32."
            )
            array = array.astype("float32")
            return self._scouter.create_monitor_profile_f32(array=array)

        return self._scouter.create_monitor_profile_f64(array=array)

    def create_monitoring_profile(
        self, data: Union[pd.DataFrame, NDArray]
    ) -> MonitoringProfile:
        return MonitoringProfile.model_validate_json(
            self._get_monitoring_profile(data),
        )
