import datetime
from typing import Any, Dict, List, Optional

from .. import DriftType

class HTTPConfig:
    server_url: str
    use_auth: bool
    username: str
    password: str
    auth_token: str

    def __init__(
        self,
        server_url: Optional[str] = None,
        use_auth: bool = False,
        username: Optional[str] = None,
        password: Optional[str] = None,
        auth_token: Optional[str] = None,
    ) -> None:
        """HTTP configuration to use with the HTTPProducer.

        Args:
            server_url:
                URL of the HTTP server to publish messages to.
                If not provided, the value of the HTTP_SERVER_URL environment variable is used.

            use_auth:
                Whether to use basic authentication.
                Default is False.

            username:
                Username for basic authentication.

            password:
                Password for basic authentication.

            auth_token:
                Authorization token to use for authentication.

        """

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
        time_interval: TimeInterval,
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
            time_interval:
                Time window for drift request
            max_data_points:
                Maximum data points to return
            drift_type:
                Drift type for request
        """

# Client
class ScouterClient:
    """Helper client for interacting with Scouter Server"""

    def __init__(self, config: Optional[HTTPConfig] = None) -> None:
        """Initialize ScouterClient

        Args:
            config:
                HTTP configuration for interacting with the server.
        """

    def get_binned_drift(self, drift_request: DriftRequest) -> Any:
        """Get drift map from server

        Args:
            drift_request:
                DriftRequest object

        Returns:
            Drift map of type BinnedCustomMetrics | BinnedPsiFeatureMetrics | BinnedSpcFeatureMetrics
        """
        
    def register_profile(self, profile: Any) -> None:
        """Insert drift profile into server

        Args:
            profile:
                Drift profile
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
    features: Dict[str, SpcDriftFeature]
