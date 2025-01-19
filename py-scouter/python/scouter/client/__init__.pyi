import datetime
from typing import Any, Dict, List

from .. import DriftType
from ..queue import HTTPConfig

class SpcFeatureResult:
    feature: str
    created_at: List[datetime.datetime]
    values: List[float]

class TimeInterval:
    FiveMinutes: "TimeInterval"
    FifteenMinutes: "TimeInterval"
    ThirtyMinutes: "TimeInterval"
    OneHour: "TimeInterval"
    ThreeHours: "TimeInterval"
    SixHours: "TimeInterval"
    TwelveHours: "TimeInterval"
    TwentyFourHours: "TimeInterval"
    TwoDays: "TimeInterval"
    FiveDays: "TimeInterval"

class DriftRequest:
    def __init__(
        self,
        name: str,
        repository: str,
        version: str,
        time_window: TimeInterval,
        max_data_points: int,
        drift_type: DriftType,
    ) -> None:
        """Initialize drift request

        Args:
            name:
                Model name
            repository:
                Model repository
            version:
                Model version
            time_window:
                Time window for drift request
            max_data_points:
                Maximum data points to return
            drift_type:
                Drift type for request
        """

# Client
class ScouterClient:
    """Helper client for interacting with Scouter Server"""

    def __init__(self, config: HTTPConfig) -> None:
        """Initialize ScouterClient

        Args:
            config:
                HTTPConfig object
        """

    def get_binned_drift(self, drift_request) -> Any:
        """Get drift map from server

        Args:
            drift_request:
                DriftRequest object
        """

class BinnedCustomMetricStats:
    avg: float
    lower_bound: float
    upper_bound: float

class BinnedCustomMetric:
    metric: str
    created_at: List[datetime.datetime]
    stats: List[BinnedCustomMetricStats]

class BinnedCustomMetrics:
    metrics: Dict[str, BinnedCustomMetric]

class BinnedPsiMetric:
    created_at: List[datetime.datetime]
    psi: List[float]
    overall_psi: float
    bins: Dict[int, float]

class BinnedPsiFeatureMetrics:
    features: Dict[str, BinnedCustomMetric]

class SpcDriftFeature:
    created_at: List[datetime.datetime]
    values: List[float]

class BinnedSpcFeatureMetrics:
    features: Dict[str, BinnedCustomMetric]
