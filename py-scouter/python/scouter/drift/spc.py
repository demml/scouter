"""This module contains the helper class for the SPC Drifter."""

from typing import List

from numpy.typing import NDArray
from scouter.drift.base import DriftHelperBase, Profile
from scouter.utils.logger import ScouterLogger
from scouter.utils.type_converter import ArrayData

from .._scouter import (  # pylint: disable=no-name-in-module
    DriftType,
    SpcAlertRule,
    SpcDriftConfig,
    SpcDrifter,
    SpcDriftProfile,
    SpcFeatureAlerts,
)

logger = ScouterLogger.get_logger()


class SpcDriftHelper(DriftHelperBase):
    def __init__(self) -> None:
        """
        Scouter class for creating monitoring profiles and detecting drift. This class will
        create a drift profile from a dataset and detect drift from new data. This
        class is primarily used to setup and actively monitor data drift
        """

        self._rusty_drifter = SpcDrifter()

    @property
    def _drifter(self) -> SpcDrifter:
        return self._rusty_drifter

    def create_string_drift_profile(
        self,
        features: list[str],
        array: list[list[str]],
        drift_config: SpcDriftConfig,
    ) -> SpcDriftProfile:
        return self._drifter.create_string_drift_profile(
            features=features,
            array=array,
            drift_config=drift_config,
        )

    def create_numeric_drift_profile(
        self, array: ArrayData, drift_config: SpcDriftConfig, bits: str
    ) -> SpcDriftProfile:
        numeric_profile = getattr(self._rusty_drifter, f"create_numeric_drift_profile_f{bits}")(
            features=array.numeric_features,
            array=array.numeric_array,
            drift_config=drift_config,
        )

        return numeric_profile

    def concat_profiles(self, profiles: list[Profile], config: SpcDriftConfig) -> SpcDriftProfile:
        num_profile = profiles[0]
        string_profile = profiles[1]

        assert isinstance(num_profile, SpcDriftProfile)
        assert isinstance(string_profile, SpcDriftProfile)

        return SpcDriftProfile(
            features={**num_profile.features, **string_profile.features},
            config=config,
        )

    def generate_alerts(
        self,
        drift_array: NDArray,
        features: List[str],
        alert_rule: SpcAlertRule,
    ) -> SpcFeatureAlerts:
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
            return self._rusty_drifter.generate_alerts(
                drift_array,
                features,
                alert_rule,
            )

        except Exception as exc:
            logger.error(f"Failed to generate alerts: {exc}")
            raise ValueError(f"Failed to generate alerts: {exc}") from exc

    @staticmethod
    def drift_type() -> DriftType:
        return DriftType.Spc
