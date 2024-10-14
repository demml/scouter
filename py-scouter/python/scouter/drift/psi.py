"""This module contains the helper class for the PSI Drifter."""

from typing import List, Optional, Union

import numpy as np
import pandas as pd
import polars as pl
import pyarrow as pa  # type: ignore
from numpy.typing import NDArray
from scouter.drift.base import DriftHelperBase
from scouter.utils.logger import ScouterLogger
from scouter.utils.type_converter import _convert_data_to_array, _get_bits

from .._scouter import (  # pylint: disable=no-name-in-module
    CommonCron,
    DriftType,
    PsiDriftConfig,
    PsiDrifter,
    PsiDriftMap,
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

        Args:
            config:
                Configuration for the drift detection. This configuration will be used to
                setup the drift profile and detect drift.

        """

        self._rusty_drifter = PsiDrifter()

    def create_drift_profile(
        self,
        data: Union[pl.DataFrame, pd.DataFrame, NDArray, pa.Table],
        config: PsiDriftConfig,
    ) -> PsiDriftProfile:
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
            logger.info("Creating drift profile.")
            string_profile = None
            numeric_profile = None
            breakpoint()
            array = _convert_data_to_array(data)

            if array.string_array is not None and array.string_features is not None:
                string_feature_names, string_features_array = self._rusty_drifter.return_dummy_data(
                    feature_names=array.string_features, features_array=array.string_array
                )

            if array.numeric_array is not None and array.numeric_features is not None:
                numeric_profile = getattr(self._rusty_drifter, f"create_numeric_drift_profile")(
                    features=array.numeric_features,
                    array=array.numeric_array,
                    drift_config=config,
                )

            if string_profile is not None and numeric_profile is not None:
                drift_profile = PsiDriftProfile(
                    features={**numeric_profile.features, **string_profile.features},
                    config=config,
                )

                return drift_profile

            profile = numeric_profile or string_profile

            assert isinstance(profile, PsiDriftProfile), "Expected DriftProfile"

            return profile

        except Exception as exc:  # type: ignore
            logger.error(f"Failed to create drift profile: {exc}")
            raise ValueError(f"Failed to create drift profile: {exc}") from exc

    def compute_drift(
        self,
        data: Union[pl.DataFrame, pd.DataFrame, NDArray, pa.Table],
        drift_profile: PsiDriftProfile,
    ) -> PsiDriftMap:
        pass

    @staticmethod
    def drift_type() -> DriftType:
        return DriftType.PSI
