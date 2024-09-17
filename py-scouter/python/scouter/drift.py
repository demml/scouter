# pylint: disable=pointless-statement,broad-exception-caught


from typing import List, Optional, Union

import numpy as np
import pandas as pd
import polars as pl
import pyarrow as pa  # type: ignore
from numpy.typing import NDArray
from scouter.utils.logger import ScouterLogger
from scouter.utils.type_converter import _convert_data_to_array, _get_bits

from ._scouter import (  # pylint: disable=no-name-in-module
    AlertRule,
    CommonCron,
    DriftConfig,
    DriftMap,
    DriftProfile,
    FeatureAlerts,
    ScouterDrifter,
)

logger = ScouterLogger.get_logger()

CommonCrons = CommonCron()  # type: ignore


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
        drift_config: Optional[DriftConfig] = None,
    ) -> DriftProfile:
        """Create a drift profile from data to use for monitoring.

        Args:
            data:
                Data to create a monitoring profile from. Data can be a numpy array,
                a polars dataframe or pandas dataframe. Data is expected to not contain
                any missing values, NaNs or infinities. These values must be removed or imputed.
                If NaNs or infinities are present, the monitoring profile will not be created.
            drift_config:
                Optional configuration for the monitoring profile. If not provided, a default
                configuration will be used.

        Returns:
            Monitoring profile
        """
        try:
            if drift_config is None:
                drift_config = DriftConfig()

            logger.info("Creating drift profile.")
            array = _convert_data_to_array(data)
            bits = _get_bits(array.numeric_array)

            string_profile: Optional[DriftProfile] = None
            numeric_profile: Optional[DriftProfile] = None

            if array.string_array is not None and array.string_features is not None:
                string_profile = self._drifter.create_string_drift_profile(
                    features=array.string_features,
                    array=array.string_array,
                    drift_config=drift_config,
                )
                assert string_profile.config.feature_map is not None
                drift_config.update_feature_map(string_profile.config.feature_map)

            if array.numeric_array is not None and array.numeric_features is not None:
                numeric_profile = getattr(self._drifter, f"create_numeric_drift_profile_f{bits}")(
                    features=array.numeric_features,
                    array=array.numeric_array,
                    drift_config=drift_config,
                )

            if string_profile is not None and numeric_profile is not None:
                drift_profile = DriftProfile(
                    features={**numeric_profile.features, **string_profile.features},
                    config=drift_config,
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
            return self._drifter.generate_alerts(
                drift_array,
                features,
                alert_rule,
            )

        except Exception as exc:
            logger.error(f"Failed to generate alerts: {exc}")
            raise ValueError(f"Failed to generate alerts: {exc}") from exc
