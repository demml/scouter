from enum import Enum
from typing import List, Optional, Union, Tuple, Dict

import pandas as pd
import polars as pl
import numpy as np
from numpy.typing import NDArray
from scouter.utils.logger import ScouterLogger

from ._scouter import (  # pylint: disable=no-name-in-module
    DataProfile,
    DriftMap,
    DriftProfile,
    ScouterDrifter,
    ScouterProfiler,
    MonitorConfig,
    Alert,
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


class ScouterBase:
    def _convert_data_to_array(
        self, data: Union[pd.DataFrame, pl.DataFrame, NDArray]
    ) -> NDArray:
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
        if features is not None:
            return features

        if isinstance(data, pl.DataFrame):
            return data.columns
        if isinstance(data, pd.DataFrame):
            columns = list(data.columns)
            return [str(i) for i in columns]
        return [f"feature_{i}" for i in range(data.shape[1])]

    def _preprocess(
        self,
        features: Optional[List[str]],
        data: Union[pl.DataFrame, pd.DataFrame, NDArray],
    ) -> Tuple[NDArray, List[str], str]:
        try:
            array = self._convert_data_to_array(data)
            features = self._get_feature_names(features, data)

            dtype = str(array.dtype)

            if dtype in [
                DataType.INT8.value,
                DataType.INT16.value,
                DataType.INT32.value,
                DataType.INT64.value,
            ]:
                logger.warning(
                    "Scouter only supports float32 and float64 arrays. Converting integer array to float32."
                )
                array = array.astype("float32")

                return array, features, DataType.str_to_bits("float32")

            return array, features, DataType.str_to_bits(dtype)

        except KeyError as exc:
            raise ValueError(f"Unsupported data type: {dtype}") from exc


class Profiler(ScouterBase):
    def __init__(self) -> None:
        """Scouter class for creating data profiles. This class will generate
        baseline statistics for a given dataset."""
        self._profiler = ScouterProfiler()

    def create_data_profile(
        self,
        data: Union[pl.DataFrame, pd.DataFrame, NDArray],
        features: Optional[List[str]] = None,
        bin_size: int = 20,
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
            bin_size:
                Optional bin size for histograms. Defaults to 20 bins.

        Returns:
            Monitoring profile
        """
        try:
            logger.info("Creating data profile.")
            array, features, bits = self._preprocess(features, data)

            profile = getattr(self._profiler, f"create_data_profile_f{bits}")(
                features=features,
                array=array,
                bin_size=bin_size,
            )

            assert isinstance(
                profile, DataProfile
            ), f"Expected DataProfile, got {type(profile)}"
            return profile

        except Exception as exc:  # type: ignore
            logger.error(f"Failed to create data profile: {exc}")
            raise ValueError(f"Failed to create data profile: {exc}") from exc


class Drifter(ScouterBase):
    def __init__(self) -> None:
        """
        Scouter class for creating monitoring profiles and detecting drift. This class will
        create a monitoring profile from a dataset and detect drift from new data. This
        class is primarily used to setup and actively monitor data drift"""

        self._drifter = ScouterDrifter()

    def create_drift_profile(
        self,
        data: Union[pl.DataFrame, pd.DataFrame, NDArray],
        monitor_config: MonitorConfig,
        features: Optional[List[str]] = None,
    ) -> DriftProfile:
        """Create a drift profile from data to use for monitoring.

        Args:
            features:
                Optional list of feature names. If not provided, feature names will be
                automatically generated.
            data:
                Data to create a monitoring profile from. Data can be a numpy array,
                a polars dataframe or pandas dataframe. Data is expected to not contain
                any missing values, NaNs or infinities. These values must be removed or imputed.
                If NaNs or infinities are present, the monitoring profile will not be created.
            monitor_config:
                Configuration for the monitoring profile.

        Returns:
            Monitoring profile
        """
        try:
            logger.info("Creating drift profile.")
            array, features, bits = self._preprocess(features, data)

            profile = getattr(self._drifter, f"create_drift_profile_f{bits}")(
                features=features,
                array=array,
                monitor_config=monitor_config,
            )

            assert isinstance(
                profile, DriftProfile
            ), f"Expected DriftProfile, got {type(profile)}"
            return profile

        except Exception as exc:  # type: ignore
            logger.error(f"Failed to create monitoring profile: {exc}")
            raise ValueError(f"Failed to create monitoring profile: {exc}") from exc

    def compute_drift(
        self,
        data: Union[pl.DataFrame, pd.DataFrame, NDArray],
        drift_profile: DriftProfile,
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
            drift_profile:
                Monitoring profile containing feature drift profiles.

        """
        try:
            logger.info("Computing drift")
            array, features, bits = self._preprocess(features, data)

            drift_map = getattr(self._drifter, f"compute_drift_f{bits}")(
                features=features,
                drift_array=array,
                drift_profile=drift_profile,
            )

            assert isinstance(
                drift_map, DriftMap
            ), f"Expected DriftMap, got {type(drift_map)}"

            return drift_map

        except KeyError as exc:
            logger.error(f"Failed to compute drift: {exc}")
            raise ValueError(f"Failed to compute drift: {exc}") from exc

    def generate_alerts(
        self, drift_array: NDArray, features: List[str], alert_rule: str
    ) -> Dict[str, Tuple[Alert, Dict[int, List[List[int]]]]]:
        """Generate alerts from a drift array and features.

        Args:
            drift_array:
                Array of drift values.
            features:
                List of feature names. Must match the order of the drift array.
            alert_rule:
                Alert rule to apply to drift values.

        Returns:
            Dictionary of alerts.
        """

        try:
            return self._drifter.generate_alerts(drift_array, features, alert_rule)

        except Exception as exc:
            logger.error(f"Failed to generate alerts: {exc}")
            raise ValueError(f"Failed to generate alerts: {exc}") from exc


class MonitorQueue:
    def __init__(self, drift_profile: DriftProfile) -> None:
        self._monitor = ScouterDrifter()
        self._drift_profile = drift_profile
        self.items: Dict[str, List[float]] = {
            feature: [] for feature in self._drift_profile.features.keys()
        }

    def insert(self, data: Dict[str, float]) -> Optional[DriftMap]:
        for feature, value in data.items():
            self.items[feature].append(value)

        self._count += 1

        if self._count >= self._drift_profile.config.sample_size:
            return self.dequeue()

        return None

    def _clear(self) -> None:
        self.items = {feature: [] for feature in self.items.keys()}
        self._count = 0

    def dequeue(self) -> DriftMap:
        try:
            # create array from items
            data = list(self.items.values())
            features = list(self.items.keys())
            array = np.array(data, dtype=np.float32).T

            drift_map = self._monitor.compute_drift_f32(
                features,
                array,
                self._drift_profile,
            )

            # clear items
            self._clear()

            return drift_map

        except Exception as exc:
            logger.error(f"Failed to compute drift: {exc}")
            raise ValueError(f"Failed to compute drift: {exc}") from exc
