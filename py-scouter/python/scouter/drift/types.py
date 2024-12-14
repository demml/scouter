from typing import Optional, Union

from .._scouter import (  # pylint: disable=no-name-in-module
    CustomComparisonMetric,
    CustomDrifter,
    CustomDriftProfile,
    CustomMetricDriftConfig,
    CustomThresholdMetric,
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


class CustomMetricData:
    def __init__(
        self,
        comparison_metrics: Optional[Union[CustomComparisonMetric, list[CustomComparisonMetric]]] = None,
        threshold_metrics: Optional[Union[CustomThresholdMetric, list[CustomThresholdMetric]]] = None,
    ) -> None:
        self.comparison_metrics = self._assign_comparison_metrics(comparison_metrics)
        self.threshold_metrics = self._assign_threshold_metrics(threshold_metrics)

    @staticmethod
    def _assign_comparison_metrics(
        comparison_metrics: Optional[Union[CustomComparisonMetric, list[CustomComparisonMetric]]]
    ) -> Optional[list[CustomComparisonMetric]]:
        if isinstance(comparison_metrics, CustomComparisonMetric):
            return [comparison_metrics]
        return comparison_metrics

    @staticmethod
    def _assign_threshold_metrics(
        threshold_metrics: Optional[Union[CustomThresholdMetric, list[CustomThresholdMetric]]]
    ) -> Optional[list[CustomThresholdMetric]]:
        if isinstance(threshold_metrics, CustomThresholdMetric):
            return [threshold_metrics]
        return threshold_metrics
