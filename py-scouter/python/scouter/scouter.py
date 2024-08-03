# pylint: disable=pointless-statement,broad-exception-caught

from functools import cached_property
from typing import Any, Dict, List, Optional, Union

import numpy as np
import pandas as pd
import polars as pl
import pyarrow as pa  # type: ignore
from numpy.typing import NDArray
from scouter.integrations.base import BaseProducer
from scouter.integrations.http import HTTPConfig
from scouter.integrations.kafka import KafkaConfig
from scouter.integrations.producer import DriftRecordProducer
from scouter.utils.logger import ScouterLogger
from scouter.utils.type_converter import _convert_data_to_array, _get_bits

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


class Drifter:
    def __init__(self) -> None:
        """
        Scouter class for creating monitoring profiles and detecting drift. This class will
        create a monitoring profile from a dataset and detect drift from new data. This
        class is primarily used to setup and actively monitor data drift"""

        self._drifter = ScouterDrifter()

    def create_drift_profile(
        self,
        data: Union[pl.DataFrame, pd.DataFrame, NDArray, pa.Table],
        monitor_config: DriftConfig,
    ) -> DriftProfile:
        """Create a drift profile from data to use for monitoring.

        Args:
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
            array = _convert_data_to_array(data)
            bits = _get_bits(array.numeric_array)

            string_profile: Optional[DriftProfile] = None
            numeric_profile: Optional[DriftProfile] = None

            if array.string_array is not None and array.string_features is not None:
                string_profile = self._drifter.create_string_drift_profile(
                    features=array.string_features,
                    array=array.string_array,
                    monitor_config=monitor_config,
                )
                assert string_profile.config.feature_map is not None
                monitor_config.update_feature_map(string_profile.config.feature_map)

            if array.numeric_array is not None and array.numeric_features is not None:
                numeric_profile = getattr(self._drifter, f"create_numeric_drift_profile_f{bits}")(
                    features=array.numeric_features,
                    array=array.numeric_array,
                    monitor_config=monitor_config,
                )

            if string_profile is not None and numeric_profile is not None:
                drift_profile = DriftProfile(
                    features={**numeric_profile.features, **string_profile.features},
                    config=monitor_config,
                )

                return drift_profile

            profile = numeric_profile or string_profile

            assert isinstance(profile, DriftProfile), "Expected DriftProfile"

            return profile

        except Exception as exc:  # type: ignore
            logger.error(f"Failed to create drift profile: {exc}")
            raise ValueError(f"Failed to create drift profile: {exc}") from exc

    def compute_drift(
        self,
        data: Union[pl.DataFrame, pd.DataFrame, NDArray, pa.Table],
        drift_profile: DriftProfile,
    ) -> DriftMap:
        """Compute drift from data and monitoring profile.

        Args:
            data:
                Data to compute drift from. Data can be a numpy array,
                a polars dataframe or pandas dataframe. Data is expected to not contain
                any missing values, NaNs or infinities.
            drift_profile:
                Monitoring profile containing feature drift profiles.

        """
        try:
            logger.info("Computing drift")
            array = _convert_data_to_array(data)
            bits = _get_bits(array.numeric_array)

            if array.string_array is not None and array.string_features is not None:
                string_array: NDArray = getattr(self._drifter, f"convert_strings_to_numpy_f{bits}")(
                    array=array.string_array,
                    features=array.string_features,
                    drift_profile=drift_profile,
                )

                if array.numeric_array is not None and array.numeric_features is not None:
                    array.numeric_array = np.concatenate((array.numeric_array, string_array), axis=1)

                    array.numeric_features += array.string_features

                else:
                    array.numeric_array = string_array
                    array.numeric_features = array.string_features

            drift_map = getattr(self._drifter, f"compute_drift_f{bits}")(
                features=array.numeric_features,
                drift_array=array.numeric_array,
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
        self._set_cached_properties()

    def _set_cached_properties(self) -> None:
        """Calls the cached properties to initialize the cache.
        Run during initialization to avoid lazy loading of properties during
        the first call.
        """
        logger.info("Initializing cache")
        self.mapped_features
        self.feature_map
        self.feature_names
        logger.info("Cache initialized")

    @cached_property
    def mapped_features(self) -> List[str]:
        """List of features that will need to be mapped to a numeric representation.
        This is precomputed during drift profile creation.
        """
        if self._drift_profile.config.feature_map is None:
            logger.info("Drift profile does not contain a feature map.")
            return []
        return list(self._drift_profile.config.feature_map.features.keys())

    @cached_property
    def feature_map(self) -> Dict[str, Dict[str, int]]:
        """Feature map from the drift profile. Used to map string values for a
        categorical feature to a numeric representation."""
        if self._drift_profile.config.feature_map is None:
            logger.warning("Feature map not found in drift profile. Returning empty map.")
            return {}
        return self._drift_profile.config.feature_map.features

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
        try:
            for feature, value in data.items():
                # attempt to map string values to numeric representation
                # fallback to missing value if not found. This is computed during
                # drift profile creation.
                if feature in self.mapped_features:
                    value = self.feature_map[feature].get(value, self.feature_map[feature]["missing"])

                self.feature_queue[feature].append(value)

            self._count += 1

            if self._count >= self._drift_profile.config.sample_size:
                return self.publish()

            return None
        except Exception as exc:
            logger.error("Failed to insert data into monitoring queue: {}. Passing", exc)
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
            logger.error("Failed to compute drift: {}", exc)
            raise ValueError(f"Failed to compute drift: {exc}") from exc
