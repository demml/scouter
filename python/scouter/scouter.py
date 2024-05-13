from typing import Union, Optional, List
from numpy.typing import NDArray
import polars as pl

# import polars as pl
import pandas as pd
from scouter.utils.logger import ScouterLogger

from ._scouter import RustScouter, MonitorProfile  # pylint: disable=no-name-in-module

logger = ScouterLogger.get_logger()


class Scouter:
    def __init__(self) -> None:
        """
        Scouter generates data profiles and monitoring profiles from arrays. Accepted
        array types include numpy arrays, polars dataframes and pandas dataframes.


        Args:
            feature_names:
                Optional list of feature names.
        """
        self._scouter = RustScouter()

    def _convert_data_to_array(self, data: Union[pd.DataFrame, pl.DataFrame, NDArray]) -> NDArray:
        """Convert data to numpy array.

        Args:
            data:
                Data to convert to numpy array.

        Returns:
            Numpy array
        """
        if isinstance(data, pl.DataFrame):
            return data.to_numpy()
        if isinstance(data, pd.DataFrame):
            return data.to_numpy()
        return data

    def _get_feature_names(
        self,
        features: Optional[List[str]],
        data: Union[pd.DataFrame, pl.DataFrame, NDArray],
    ) -> List[str]:
        """Check if feature names are provided. If not, generate feature names.

        Args:
            features:
                Optional list of feature names.
            data:
                Data to generate feature names from.
        """
        if features is not None:
            return features

        if isinstance(data, pl.DataFrame):
            return data.columns
        if isinstance(data, pd.DataFrame):
            columns = list(data.columns)
            return [str(i) for i in columns]
        return [f"feature_{i}" for i in range(data.shape[1])]

    def _get_monitoring_profile(
        self,
        features: Optional[List[str]],
        data: Union[pd.DataFrame, NDArray],
    ) -> MonitorProfile:
        # convert data to numpy array
        array = self._convert_data_to_array(data)
        features = self._get_feature_names(features, data)

        # get numpy array type
        dtype = array.dtype

        # check if numpy array is float32
        if dtype == "float32":
            return self._scouter.create_monitor_profile_f32(
                features=features,
                array=array,
            )

        if dtype == "float64":
            return self._scouter.create_monitor_profile_f64(
                features=features,
                array=array,
            )

        # if numpy array is integer, convert to float32
        if dtype in ["int8", "int16", "int32", "int64"]:
            logger.warning("Scouter only supports float32 and float64 arrays. Converting integer array to float32.")
            array = array.astype("float32")
            return self._scouter.create_monitor_profile_f32(
                features=features,
                array=array,
            )

        raise ValueError(f"Unsupported numpy array type. Cannot create monitoring profile: {dtype}")

    def create_monitoring_profile(
        self,
        data: Union[pl.DataFrame, pd.DataFrame, NDArray],
        features: Optional[List[str]] = None,
    ) -> MonitorProfile:
        """Create a monitoring profile from data.

        Args:
            features:
                Optional list of feature names. If not provided, feature names will be
                automatically generated.
            data:
                Data to create a monitoring profile from. Data can be a numpy array,
                a polars dataframe or pandas dataframe. Data is expected to not contain
                any missing values, NaNs or infinities. These values must be removed or imputed.
                If NaNs or infinities are present, the monitoring profile will not be created.

        Returns:
            Monitoring profile
        """
        try:
            return self._get_monitoring_profile(features, data)
        except Exception as exc:  # type: ignore
            logger.error(f"Failed to create monitoring profile: {exc}")
            raise ValueError(f"Failed to create monitoring profile: {exc}") from exc
