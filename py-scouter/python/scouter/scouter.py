from enum import Enum
from functools import cached_property
from typing import Any, Dict, List, Optional, Tuple, Union

import numpy as np
import pandas as pd
import polars as pl
from numpy.typing import NDArray
from scouter.integrations.base import BaseProducer
from scouter.integrations.http import HTTPConfig
from scouter.integrations.kafka import KafkaConfig
from scouter.integrations.producer import DriftRecordProducer
from scouter.utils.logger import ScouterLogger

from ._scouter import (  # pylint: disable=no-name-in-module
    AlertRule,
    CommonCron,
    DataProfile,
    DriftConfig,
    DriftMap,
    DriftProfile,
    DriftServerRecord,
    FeatureAlerts,
    ScouterDrifter,
    ScouterProfiler,
)

logger = ScouterLogger.get_logger()

CommonCrons = CommonCron()  # type: ignore


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
    def _convert_data_to_array(self, data: Union[pd.DataFrame, pl.DataFrame, NDArray]) -> NDArray:
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
                logger.warning("Scouter only supports float32 and float64 arrays. Converting integer array to float32.")
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

            assert isinstance(profile, DataProfile), f"Expected DataProfile, got {type(profile)}"
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
        monitor_config: DriftConfig,
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

            assert isinstance(profile, DriftProfile), f"Expected DriftProfile, got {type(profile)}"
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

            assert isinstance(drift_map, DriftMap), f"Expected DriftMap, got {type(drift_map)}"

            return drift_map

        except KeyError as exc:
            logger.error(f"Failed to compute drift: {exc}")
            raise ValueError(f"Failed to compute drift: {exc}") from exc

    def generate_alerts(
        self,
        drift_array: NDArray,
        features: List[str],
        alert_rule: AlertRule,
    ) -> FeatureAlerts:
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
    def __init__(
        self,
        drift_profile: DriftProfile,
        config: Union[KafkaConfig, HTTPConfig],
    ) -> None:
        """Instantiate a monitoring queue to monitor data drift.

        Args:
            drift_profile:
                Monitoring profile containing feature drift profiles.
            config:
                Configuration for the monitoring producer. The configured producer
                will be used to publish drift records to the monitoring server.
        """
        self._monitor = ScouterDrifter()
        self._drift_profile = drift_profile

        self.feature_queue: Dict[str, List[float]] = {feature: [] for feature in self.feature_names}
        self._count = 0

        self._producer = self._get_producer(config)

    @cached_property
    def feature_names(self) -> List[str]:
        """Feature names in the monitoring profile."""
        return list(self._drift_profile.features.keys())

    def _get_producer(self, config: Union[KafkaConfig, HTTPConfig]) -> BaseProducer:
        """Get the producer based on the configuration."""
        return DriftRecordProducer.get_producer(config)

    def insert(self, data: Dict[Any, Any]) -> Optional[List[DriftServerRecord]]:
        """Insert data into the monitoring queue.

        Args:
            data:
                Dictionary of feature values to insert into the monitoring queue.

        Returns:
            List of drift records if the monitoring queue has enough data to compute
        """
        for feature, value in data.items():
            self.feature_queue[feature].append(value)

        self._count += 1

        if self._count >= self._drift_profile.config.sample_size:
            return self.publish()

        return None

    def _clear_queue(self) -> None:
        """Clear the monitoring queue."""
        self.feature_queue = {feature: [] for feature in self.feature_names}
        self._count = 0

    def publish(self) -> List[DriftServerRecord]:
        """Publish drift records to the monitoring server."""
        try:
            # create array from items
            data = list(self.feature_queue.values())
            array = np.array(data, dtype=np.float64).T

            drift_records = self._monitor.sample_data_f64(self.feature_names, array, self._drift_profile)

            for record in drift_records:
                self._producer.publish(record)

            # clear items
            self._clear_queue()

            return drift_records

        except Exception as exc:
            logger.error(f"Failed to compute drift: {exc}")
            raise ValueError(f"Failed to compute drift: {exc}") from exc
