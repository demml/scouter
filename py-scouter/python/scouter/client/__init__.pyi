import datetime
from typing import List

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

    def get_drift(self, drift_request) -> List[SpcFeatureResult]:
        """Get drift map from server

        Args:
            drift_request:
                DriftRequest object
        """
