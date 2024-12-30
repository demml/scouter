"""This module contains the helper class for the SPC Drifter."""

from typing import Any, List, Optional, Union

import numpy as np
import pandas as pd
import polars as pl
import pyarrow as pa  # type: ignore
from numpy.typing import NDArray
from scouter.utils.logger import ScouterLogger
from scouter.utils.type_converter import ArrayData, _convert_data_to_array, _get_bits

from .._scouter import (  # pylint: disable=no-name-in-module
    CustomDrifter,
    CustomDriftProfile,
    CustomMetric,
    CustomMetricDriftConfig,
    DriftType,
    PsiDriftConfig,
    PsiDrifter,
    PsiDriftMap,
    PsiDriftProfile,
    SpcDriftConfig,
    SpcDrifter,
    SpcDriftMap,
    SpcDriftProfile,
)

Profile = Union[SpcDriftProfile, PsiDriftProfile, CustomDriftProfile]
Config = Union[SpcDriftConfig, PsiDriftConfig, CustomMetricDriftConfig]
DriftMap = Union[SpcDriftMap, PsiDriftMap]
Drifter = Union[SpcDrifter, PsiDrifter, CustomDrifter]
CustomMetricData = Union[CustomMetric, list[CustomMetric]]


logger = ScouterLogger.get_logger()


class DriftHelperBase:
    """Base class for drift helper classes."""

    def __init__(self) -> None:
        raise NotImplementedError

    @property
    def _drifter(self) -> Drifter:
        raise NotImplementedError

    def create_string_drift_profile(self, features: List[str], array: List[List[str]], drift_config: Any) -> Profile:
        raise NotImplementedError

    def create_numeric_drift_profile(self, array: ArrayData, drift_config: Any, bits: str) -> Profile:
        raise NotImplementedError

    def concat_profiles(self, profiles: List[Profile], config: Any) -> Profile:
        raise NotImplementedError

    def convert_string_to_numpy(
        self,
        string_array: List[List[str]],
        features: List[str],
        drift_profile: Profile,
        bits: str,
    ) -> NDArray:
        """Convert string array to numpy array using the drift profile.

        Args:
            string_array:
                Array of strings to convert to numpy array.
            features:
                List of feature names.
            drift_profile:
                Drift profile to use for conversion.
            bits:
                Number of bits to use for conversion.

        Returns:
            Numpy array of converted strings.
        """

        converted_array: NDArray = getattr(self._drifter, f"convert_strings_to_numpy_f{bits}")(
            array=string_array,
            features=features,
            drift_profile=drift_profile,
        )

        return converted_array

    def _compute_drift(
        self,
        features: List[str],
        numeric_array: NDArray,
        drift_profile: Profile,
        bits: str,
    ) -> DriftMap:
        """Compute drift from data and monitoring profile.

        Args:
            features:
                List of feature names.
            numeric_array:
                Array of numeric values to compute drift from.
            drift_profile:
                Monitoring profile containing feature drift profiles.
            bits:
                Number of bits to use for computation.

        Returns:
            Drift map

        """
        drift_map: DriftMap = getattr(self._drifter, f"compute_drift_f{bits}")(
            features=features,
            array=numeric_array,
            drift_profile=drift_profile,
        )

        return drift_map

    def create_drift_profile(
        self,
        data: Union[pl.DataFrame, pd.DataFrame, NDArray, pa.Table, CustomMetricData],
        config: Config,
    ) -> Profile:
        """Create a drift profile from data to use for monitoring.

        Args:
            data:
                Data to create a monitoring profile from. Data can be a numpy array,
                a polars dataframe or pandas dataframe. Data is expected to not contain
                any missing values, NaNs or infinities. These values must be removed or imputed.
                If NaNs or infinities are present, the monitoring profile will not be created.
            config:
                Configuration for the drift detection. This configuration will be used to
                setup the drift profile and detect drift.

        Returns:
            Monitoring profile
        """
        try:
            assert isinstance(
                config, (SpcDriftConfig, PsiDriftConfig)
            ), f"{type(config)} was detected, when was expected"
            logger.info("Creating drift profile.")
            array = _convert_data_to_array(data)
            bits = _get_bits(array.numeric_array)

            string_profile: Optional[Profile] = None
            numeric_profile: Optional[Profile] = None

            if array.string_array is not None and array.string_features is not None:
                string_profile = self.create_string_drift_profile(
                    features=array.string_features,
                    array=array.string_array,
                    drift_config=config,
                )
                assert isinstance(
                    string_profile.config, (SpcDriftConfig, PsiDriftConfig)
                ), f"{type(config)} was detected, when was expected"
                assert string_profile.config.feature_map is not None
                config.update_feature_map(string_profile.config.feature_map)

            if array.numeric_array is not None and array.numeric_features is not None:
                numeric_profile = self.create_numeric_drift_profile(
                    array=array,
                    drift_config=config,
                    bits=bits,
                )

            if string_profile is not None and numeric_profile is not None:
                drift_profile = self.concat_profiles(
                    profiles=[numeric_profile, string_profile],
                    config=config,
                )

                return drift_profile

            profile = numeric_profile or string_profile

            assert isinstance(profile, (SpcDriftProfile, PsiDriftProfile)), "Expected DriftProfile"

            return profile
        except Exception as exc:  # type: ignore
            logger.error(f"Failed to create drift profile: {exc}")
            raise ValueError(f"Failed to create drift profile: {exc}") from exc

    def compute_drift(
        self,
        data: Union[pl.DataFrame, pd.DataFrame, NDArray, pa.Table],
        drift_profile: Profile,
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
                string_array = self.convert_string_to_numpy(
                    string_array=array.string_array,
                    features=array.string_features,
                    drift_profile=drift_profile,
                    bits=bits,
                )

                if array.numeric_array is not None and array.numeric_features is not None:
                    array.numeric_array = np.concatenate((array.numeric_array, string_array), axis=1)

                    array.numeric_features += array.string_features

                else:
                    array.numeric_array = string_array
                    array.numeric_features = array.string_features

            assert array.numeric_array is not None, "Numeric array is None"
            assert array.numeric_features is not None, "Numeric features are None"

            drift_map = self._compute_drift(
                features=array.numeric_features,
                numeric_array=array.numeric_array,
                drift_profile=drift_profile,
                bits=bits,
            )

            assert isinstance(drift_map, (SpcDriftMap, PsiDriftMap)), f"Expected DriftMap, got {type(drift_map)}"

            return drift_map

        except KeyError as exc:
            logger.error(f"Failed to compute drift: {exc}")
            raise ValueError(f"Failed to compute drift: {exc}") from exc

    def generate_alerts(
        self,
        drift_array: NDArray,
        features: list[str],
        alert_rule: Any,
    ) -> Any:
        raise NotImplementedError

    @staticmethod
    def drift_type() -> DriftType:
        raise NotImplementedError


def get_drift_helper(drift_type: DriftType) -> DriftHelperBase:
    """Helper function to get the correct drift helper based on the drift type."""

    converter = next(
        (converter for converter in DriftHelperBase.__subclasses__() if converter.drift_type() == drift_type),
        None,
    )

    if converter is None:
        raise ValueError(f"Unsupported drift type: {drift_type.value}")

    return converter()
