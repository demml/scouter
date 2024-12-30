"""This module contains the helper class for the Custom Drifter."""

from typing import Any, Union

import pandas as pd
import polars as pl
import pyarrow as pa  # type: ignore
from numpy.typing import NDArray
from scouter.drift.base import Config, CustomMetricData, DriftHelperBase, Profile
from scouter.utils.logger import ScouterLogger
from scouter.utils.type_converter import ArrayData

from .._scouter import (  # pylint: disable=no-name-in-module
    CustomDrifter,
    CustomMetric,
    CustomMetricDriftConfig,
    DriftType,
)

logger = ScouterLogger.get_logger()


class CustomDriftHelper(DriftHelperBase):
    def __init__(self) -> None:
        """
        Scouter class for creating monitoring profiles and detecting drift. This class will
        create a drift profile from a dataset and detect drift from new data. This
        class is primarily used to setup and actively monitor data drift
        """

        self._rusty_drifter = CustomDrifter()

    @property
    def _drifter(self) -> CustomDrifter:
        return self._rusty_drifter

    def create_string_drift_profile(self, features: list[str], array: list[list[str]], drift_config: Config) -> Profile:
        raise NotImplementedError

    def create_numeric_drift_profile(self, array: ArrayData, drift_config: Config, bits: str) -> Profile:
        raise NotImplementedError

    def concat_profiles(self, profiles: list[Profile], config: Config) -> Profile:
        raise NotImplementedError

    def generate_alerts(self, drift_array: NDArray, features: list[str], alert_rule: Any) -> Any:
        raise NotImplementedError

    @staticmethod
    def drift_type() -> DriftType:
        return DriftType.Custom

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
                data, (CustomMetric, list)
            ), f"{type(data)} was detected, when CustomMetricData was expected"
            if isinstance(data, CustomMetric):
                data = [data]

            assert isinstance(
                config, CustomMetricDriftConfig
            ), f"{type(config)} was detected, CustomMetricDriftConfig when was expected"
            return self._rusty_drifter.create_drift_profile(config, data)
        except Exception as exc:  # type: ignore
            logger.error(f"Failed to create drift profile: {exc}")
            raise ValueError(f"Failed to create drift profile: {exc}") from exc
