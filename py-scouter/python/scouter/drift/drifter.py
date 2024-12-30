from typing import Any, List, Optional, Union, overload

import pandas as pd
import polars as pl
import pyarrow as pa  # type: ignore
from numpy.typing import NDArray
from scouter.drift import DriftHelperBase, get_drift_helper
from scouter.drift.base import Config, CustomMetricData, DriftMap, Profile
from scouter.utils.logger import ScouterLogger

from .._scouter import (  # pylint: disable=no-name-in-module; CustomMetricDriftConfig,
    CommonCron,
    CustomDriftProfile,
    CustomMetricDriftConfig,
    DriftType,
    PsiDriftConfig,
    PsiDriftMap,
    PsiDriftProfile,
    SpcAlertRule,
    SpcDriftConfig,
    SpcDriftMap,
    SpcDriftProfile,
    SpcFeatureAlerts,
)

logger = ScouterLogger.get_logger()

CommonCrons = CommonCron()  # type: ignore


class Drifter:
    def __init__(self, drift_type: Optional[DriftType] = None) -> None:
        """
        Scouter class for creating drift profiles and detecting drift. This class will
        create a drift profile from a dataset and detect drift from new data. This
        class is primarily used to setup and actively monitor data drift

        Args:
            drift_type:
                Type of drift to detect. Defaults to SPC drift detection.

        """
        # if drift_type == DriftType.Custom:
        #     self._custom_drift_helper = CustomDriftHelper()
        # else:
        self._drift_helper: DriftHelperBase = get_drift_helper(drift_type or DriftType.Spc)

    @overload
    def create_drift_profile(
        self,
        data: Union[pl.DataFrame, pd.DataFrame, NDArray[Any], pa.Table],
        config: SpcDriftConfig,
    ) -> SpcDriftProfile: ...

    @overload
    def create_drift_profile(
        self,
        data: Union[pl.DataFrame, pd.DataFrame, NDArray[Any], pa.Table],
        config: PsiDriftConfig,
    ) -> PsiDriftProfile: ...

    @overload
    def create_drift_profile(
        self,
        data: Union[pl.DataFrame, pd.DataFrame, NDArray[Any], pa.Table],
    ) -> SpcDriftProfile: ...

    @overload
    def create_drift_profile(
        self,
        data: CustomMetricData,
        config: CustomMetricDriftConfig,
    ) -> CustomDriftProfile: ...

    def create_drift_profile(
        self,
        data: Union[pl.DataFrame, pd.DataFrame, NDArray[Any], pa.Table, CustomMetricData],
        config: Optional[Config] = None,
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
        _config = config or SpcDriftConfig()
        assert _config.drift_type == self._drift_helper.drift_type(), (
            f"Drift type mismatch. Expected drift type: {self._drift_helper.drift_type()}, "
            f"got drift type: {_config.drift_type}"
        )
        return self._drift_helper.create_drift_profile(data, _config)

    @overload
    def compute_drift(
        self,
        data: Union[pl.DataFrame, pd.DataFrame, NDArray, pa.Table],
        drift_profile: SpcDriftProfile,
    ) -> SpcDriftMap: ...

    @overload
    def compute_drift(
        self,
        data: Union[pl.DataFrame, pd.DataFrame, NDArray, pa.Table],
        drift_profile: PsiDriftProfile,
    ) -> PsiDriftMap: ...

    def compute_drift(
        self,
        data: Union[pl.DataFrame, pd.DataFrame, NDArray, pa.Table],
        drift_profile: Profile,
    ) -> DriftMap:
        """Compute drift from data using a drift profile.

        Args:
            data:
                Data to compute drift from. Data can be a numpy array, a polars dataframe
                or pandas dataframe. Data is expected to not contain any missing values,
                NaNs or infinities. These values must be removed or imputed. If NaNs or
                infinities are present, drift will not be computed.
            drift_profile:
                Drift profile to use for computing drift.

        Returns:
            Drift map
        """
        assert drift_profile.config.drift_type == self._drift_helper.drift_type(), (
            f"Drift type mismatch. Expected drift type: {self._drift_helper.drift_type()}, "
            f"got drift type: {drift_profile.config.drift_type}"
        )

        return self._drift_helper.compute_drift(data, drift_profile)

    def generate_alerts(
        self,
        drift_array: NDArray,
        features: List[str],
        alert_rule: Union[SpcAlertRule],
    ) -> Union[SpcFeatureAlerts]:
        """Generate alerts from drift data.

        Args:
            drift_array:
                Array of drift values to generate alerts from.
            features:
                List of feature names.
            alert_rule:
                Alert rule to use for generating alerts.

        Returns:
            Feature alerts
        """
        return self._drift_helper.generate_alerts(drift_array, features, alert_rule)
