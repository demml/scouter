from pydantic import BaseModel

from .._scouter import (  # pylint: disable=no-name-in-module
    CustomComparisonMetric,
    CustomDriftProfile,
    CustomMetricDriftConfig,
    CustomThresholdMetric,
)


class CustomMetricData(BaseModel):
    comparison_metrics: list[CustomComparisonMetric]
    threshold_metrics: list[CustomThresholdMetric]


class CustomDrifter:
    @staticmethod
    def create_custom_profile(data: CustomMetricData, config: CustomMetricDriftConfig) -> CustomDriftProfile:
        return CustomDriftProfile({}, config, "1")
