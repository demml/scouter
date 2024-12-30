"""This module contains the helper class for the PSI Drifter."""

from typing import Any

from numpy.typing import NDArray
from scouter.drift.base import DriftHelperBase, Profile
from scouter.utils.logger import ScouterLogger
from scouter.utils.type_converter import ArrayData

from .._scouter import (  # pylint: disable=no-name-in-module
    CommonCron,
    DriftType,
    PsiDriftConfig,
    PsiDrifter,
    PsiDriftProfile,
)

logger = ScouterLogger.get_logger()

CommonCrons = CommonCron()  # type: ignore


class PsiDriftHelper(DriftHelperBase):
    def __init__(self) -> None:
        """
        Scouter class for creating monitoring profiles and detecting drift. This class will
        create a drift profile from a dataset and detect drift from new data. This
        class is primarily used to setup and actively monitor data drift
        """

        self._rusty_drifter = PsiDrifter()

    @property
    def _drifter(self) -> PsiDrifter:
        return self._rusty_drifter

    def create_string_drift_profile(
        self,
        features: list[str],
        array: list[list[str]],
        drift_config: PsiDriftConfig,
    ) -> PsiDriftProfile:
        return self._drifter.create_string_drift_profile(
            features=features,
            array=array,
            drift_config=drift_config,
        )

    def create_numeric_drift_profile(
        self, array: ArrayData, drift_config: PsiDriftConfig, bits: str
    ) -> PsiDriftProfile:
        numeric_profile = getattr(self._rusty_drifter, f"create_numeric_drift_profile_f{bits}")(
            features=array.numeric_features,
            array=array.numeric_array,
            drift_config=drift_config,
        )

        return numeric_profile

    def concat_profiles(self, profiles: list[Profile], config: PsiDriftConfig) -> PsiDriftProfile:
        num_profile = profiles[0]
        string_profile = profiles[1]

        assert isinstance(num_profile, PsiDriftProfile)
        assert isinstance(string_profile, PsiDriftProfile)

        return PsiDriftProfile(
            features={**num_profile.features, **string_profile.features},
            config=config,
        )

    def generate_alerts(self, drift_array: NDArray, features: list[str], alert_rule: Any) -> Any:
        raise NotImplementedError

    @staticmethod
    def drift_type() -> DriftType:
        return DriftType.Psi
