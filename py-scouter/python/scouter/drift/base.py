from typing import Union

import pandas as pd
import polars as pl
import pyarrow as pa  # type: ignore
from numpy.typing import NDArray

from .._scouter import (  # pylint: disable=no-name-in-module
    DriftType,
    SpcAlertRule,
    SpcDriftConfig,
    SpcDriftMap,
    SpcDriftProfile,
    SpcFeatureAlerts,
)


class DriftHelperBase:
    """Base class for drift helper classes."""

    def __init__(self) -> None:
        raise NotImplementedError

    def create_drift_profile(
        self,
        data: Union[pl.DataFrame, pd.DataFrame, NDArray, pa.Table],
        config: Union[SpcDriftConfig],
    ) -> Union[SpcDriftProfile]:
        raise NotImplementedError

    def compute_drift(
        self,
        data: Union[pl.DataFrame, pd.DataFrame, NDArray, pa.Table],
        drift_profile: Union[SpcDriftProfile],
    ) -> Union[SpcDriftMap]:
        raise NotImplementedError

    def generate_alerts(
        self,
        drift_array: NDArray,
        features: list[str],
        alert_rule: Union[SpcAlertRule],
    ) -> Union[SpcFeatureAlerts]:
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
