from enum import Enum
from typing import List, Optional, Union

import pandas as pd
import polars as pl
from numpy.typing import NDArray
from scouter.utils.logger import ScouterLogger

from ._scouter import (  # pylint: disable=no-name-in-module
    DataProfile,
    DriftMap,
    MonitorProfile,
    RustScouter,
)

logger = ScouterLogger.get_logger()


class DataType(str, Enum):
    FLOAT32 = "float32"
    FLOAT64 = "float64"
    INT8 = "int8"
    INT16 = "int16"
    INT32 = "int32"
    INT64 = "int64"

    @staticmethod
    def str_to_bits(dtype: str) -> str:
        bits = {
            "float32": "32",
            "float64": "64",
        }
        return bits[dtype]


class Scouter:
    def __init__(self, bin_size: Optional[int] = None) -> None:
        """
        Scouter generates data profiles and monitoring profiles from arrays. Accepted
        array types include numpy arrays, polars dataframes and pandas dataframes.


        Args:
            bin_size:
                Optional bin size for histograms. Defaults to 20 bins.
        """
        self._scouter = RustScouter(bin_size=bin_size)

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

    def _get_profile(
        self,
        features: Optional[List[str]],
        data: Union[pl.DataFrame, pd.DataFrame, NDArray],
        profile_type: str,
    ) -> Union[DataProfile, MonitorProfile]:
        # convert data to numpy array
        array = self._convert_data_to_array(data)
        features = self._get_feature_names(features, data)

        # get numpy array type
        dtype = str(array.dtype)

        if dtype in [
            DataType.INT8.value,
            DataType.INT16.value,
            DataType.INT32.value,
            DataType.INT64.value,
        ]:
            logger.warning("Scouter only supports float32 and float64 arrays. Converting integer array to float32.")
            array = array.astype("float32")
            return getattr(self._scouter, f"create_{profile_type}_profile_f32")(features=features, array=array)

        try:
            bits = DataType.str_to_bits(dtype)
            return getattr(self._scouter, f"create_{profile_type}_profile_f{bits}")(features=features, array=array)
        except KeyError as exc:
            print()
            raise ValueError(f"Unsupported data type: {dtype}") from exc

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
            logger.info("Creating monitoring profile.")
            profile = self._get_profile(features, data, "monitor")
            assert isinstance(profile, MonitorProfile)
            return profile
        except Exception as exc:  # type: ignore
            logger.error(f"Failed to create monitoring profile: {exc}")
            raise ValueError(f"Failed to create monitoring profile: {exc}") from exc

    def create_data_profile(
        self,
        data: Union[pl.DataFrame, pd.DataFrame, NDArray],
        features: Optional[List[str]] = None,
    ) -> DataProfile:
        """Create a data profile from data.

        Args:
            features:
                Optional list of feature names. If not provided, feature names will be
                automatically generated.
            data:
                Data to create a data profile from. Data can be a numpy array,
                a polars dataframe or pandas dataframe. Data is expected to not contain
                any missing values, NaNs or infinities. These values must be removed or imputed.
                If NaNs or infinities are present, the data profile will not be created.

        Returns:
            Monitoring profile
        """
        try:
            logger.info("Creating data profile.")
            profile = self._get_profile(features, data, "data")
            assert isinstance(profile, DataProfile), f"Expected DataProfile, got {type(profile)}"
            return profile
        except Exception as exc:  # type: ignore
            logger.error(f"Failed to create data profile: {exc}")
            raise ValueError(f"Failed to create data profile: {exc}") from exc

    def compute_drift(
        self,
        data: Union[pl.DataFrame, pd.DataFrame, NDArray],
        monitor_profile: MonitorProfile,
        sample: bool,
        features: Optional[List[str]] = None,
    ) -> DriftMap:
        """Compute drift from data and monitoring profile.

        Args:
            features:
                Optional list of feature names. If not provided, feature names will be
                automatically generated. Names must match the feature names in the monitoring profile.
            data:
                Data to compute drift from. Data can be a numpy array,
                a polars dataframe or pandas dataframe. Data is expected to not contain
                any missing values, NaNs or infinities.
            monitor_profile:
                Monitoring profile to compare data to.
            sample:
                If True, compute drift from a sample of the data.
        """
        try:
            # convert data to numpy array
            array = self._convert_data_to_array(data)
            features = self._get_feature_names(features, data)

            # get numpy array type
            dtype = str(array.dtype)

            if dtype in [
                DataType.INT8.value,
                DataType.INT16.value,
                DataType.INT32.value,
                DataType.INT64.value,
            ]:
                logger.warning("Scouter only supports float32 and float64 arrays. Converting integer array to float32.")
                array = array.astype("float32")
                return self._scouter.compute_drift_f32(
                    features=features,
                    array=array,
                    monitor_profile=monitor_profile,
                    sample=sample,
                )

            bits = DataType.str_to_bits(dtype)
            return getattr(self._scouter, f"compute_drift_f{bits}")(
                features=features,
                array=array,
                monitor_profile=monitor_profile,
                sample=sample,
            )
        except KeyError as exc:
            logger.error(f"Failed to compute drift: {exc}")
            raise ValueError(f"Failed to compute drift: {exc}") from exc
