import datetime
from pathlib import Path
from typing import Any, Dict, List, Optional

from ..types import DriftType
from ..transport import HTTPConfig

class Attribute:
    """Represents a key-value attribute associated with a span."""

    key: str
    value: str

class SpanEvent:
    """Represents an event within a span."""

    timestamp: datetime.datetime
    name: str
    attributes: List[Attribute]
    dropped_attributes_count: int

class SpanLink:
    """Represents a link to another span."""

    trace_id: str
    span_id: str
    trace_state: str
    attributes: List[Attribute]
    dropped_attributes_count: int

class TraceBaggageRecord:
    """Represents a single baggage record associated with a trace."""

    created_at: datetime.datetime
    trace_id: str
    scope: str
    key: str
    value: str

class TraceFilters:
    """A struct for filtering traces, generated from Rust pyclass."""

    # Read/Write properties due to #[pyo3(get, set)]
    space: Optional[str]
    name: Optional[str]
    version: Optional[str]
    service_name: Optional[str]
    has_errors: Optional[bool]
    status_code: Optional[int]
    start_time: Optional[datetime.datetime]
    end_time: Optional[datetime.datetime]
    limit: Optional[int]
    cursor_created_at: Optional[datetime.datetime]
    cursor_trace_id: Optional[str]

    def __init__(
        self,
        space: Optional[str] = None,
        name: Optional[str] = None,
        version: Optional[str] = None,
        service_name: Optional[str] = None,
        has_errors: Optional[bool] = None,
        status_code: Optional[int] = None,
        start_time: Optional[datetime.datetime] = None,
        end_time: Optional[datetime.datetime] = None,
        limit: Optional[int] = None,
        cursor_created_at: Optional[datetime.datetime] = None,
        cursor_trace_id: Optional[str] = None,
    ) -> None:
        """Initialize trace filters.

        Args:
            space:
                Model space filter
            name:
                Model name filter
            version:
                Model version filter
            service_name:
                Service name filter
            has_errors:
                Filter by presence of errors
            status_code:
                Filter by root span status code
            start_time:
                Start time boundary (UTC)
            end_time:
                End time boundary (UTC)
            limit:
                Maximum number of results to return
            cursor_created_at:
                Pagination cursor: created at timestamp
            cursor_trace_id:
                Pagination cursor: trace ID
        """

class TraceMetricBucket:
    """Represents aggregated trace metrics for a specific time bucket."""

    bucket_start: datetime.datetime
    trace_count: int
    avg_duration_ms: float
    p50_duration_ms: Optional[float]
    p95_duration_ms: Optional[float]
    p99_duration_ms: Optional[float]
    error_rate: float

class TraceListItem:
    """Represents a summary item for a trace in a list view."""

    trace_id: str
    space: str
    name: str
    version: str
    scope: str
    service_name: Optional[str]
    root_operation: Optional[str]
    start_time: datetime.datetime
    end_time: Optional[datetime.datetime]
    duration_ms: Optional[int]
    status_code: int
    status_message: Optional[str]
    span_count: Optional[int]
    has_errors: bool
    error_count: int
    created_at: datetime.datetime

class TraceSpan:
    """Detailed information for a single span within a trace."""

    # Read-only properties due to #[pyo3(get)]
    trace_id: str
    span_id: str
    parent_span_id: Optional[str]
    span_name: str
    span_kind: Optional[str]
    start_time: datetime.datetime
    end_time: Optional[datetime.datetime]
    duration_ms: Optional[int]
    status_code: str
    status_message: Optional[str]
    attributes: List[Attribute]
    events: List[SpanEvent]
    links: List[SpanLink]
    depth: int
    path: List[str]
    root_span_id: str
    span_order: int

class TracePaginationResponse:
    """Response structure for paginated trace list requests."""

    # Read-only property due to #[pyo3(get)]
    items: List[TraceListItem]

class TraceSpansResponse:
    """Response structure containing a list of spans for a trace."""

    # Read-only property due to #[pyo3(get)]
    spans: List[TraceSpan]

class TraceBaggageResponse:
    """Response structure containing trace baggage records."""

    # Read-only property due to #[pyo3(get)]
    baggage: List[TraceBaggageRecord]

class TraceMetricsRequest:
    """Request payload for fetching trace metrics."""

    # Properties (mutable by default)
    space: Optional[str]
    name: Optional[str]
    version: Optional[str]
    start_time: datetime.datetime
    end_time: datetime.datetime
    bucket_interval: str

    def __init__(
        self,
        start_time: datetime.datetime,
        end_time: datetime.datetime,
        bucket_interval: str,
        space: Optional[str] = None,
        name: Optional[str] = None,
        version: Optional[str] = None,
    ) -> None:
        """Initialize trace metrics request.

        Args:
            start_time:
                Start time boundary (UTC)
            end_time:
                End time boundary (UTC)
            bucket_interval:
                The time interval for metric aggregation buckets (e.g., '1h', '30m')
            space:
                Model space filter
            name:
                Model name filter
            version:
                Model version filter
        """

class TraceMetricsResponse:
    """Response structure containing aggregated trace metrics."""

    # Read-only property due to #[pyo3(get)]
    metrics: List[TraceMetricBucket]

class TagsResponse:
    """Response structure containing a list of tag records."""

    # Read-only property due to #[pyo3(get)]
    tags: List[TagRecord]

class TagRecord:
    """Represents a single tag record associated with an entity."""

    # Read-only properties due to #[pyo3(get)]
    created_at: datetime.datetime
    entity_type: str
    entity_id: str
    key: str
    value: str

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
        space: str,
        version: str,
        time_interval: TimeInterval,
        max_data_points: int,
        drift_type: DriftType,
    ) -> None:
        """Initialize drift request

        Args:
            name:
                Model name
            space:
                Model space
            version:
                Model version
            time_interval:
                Time window for drift request
            max_data_points:
                Maximum data points to return
            drift_type:
                Drift type for request
        """

class ProfileStatusRequest:
    def __init__(
        self, name: str, space: str, version: str, drift_type: DriftType, active: bool
    ) -> None:
        """Initialize profile status request

        Args:
            name:
                Model name
            space:
                Model space
            version:
                Model version
            drift_type:
                Profile drift type. A (repo/name/version can be associated with more than one drift type)
            active:
                Whether to set the profile as active or inactive
        """

class GetProfileRequest:
    def __init__(
        self, name: str, space: str, version: str, drift_type: DriftType
    ) -> None:
        """Initialize get profile request

        Args:
            name:
                Profile name
            space:
                Profile space
            version:
                Profile version
            drift_type:
                Profile drift type. A (repo/name/version can be associated with more than one drift type)
        """

class Alert:
    created_at: datetime.datetime
    name: str
    space: str
    version: str
    feature: str
    alert: str
    id: int
    status: str

class DriftAlertRequest:
    def __init__(
        self,
        name: str,
        space: str,
        version: str,
        active: bool = False,
        limit_datetime: Optional[datetime.datetime] = None,
        limit: Optional[int] = None,
    ) -> None:
        """Initialize drift alert request

        Args:
            name:
                Name
            space:
                Space
            version:
                Version
            active:
                Whether to get active alerts only
            limit_datetime:
                Limit datetime for alerts
            limit:
                Limit for number of alerts to return
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
            Drift map of type BinnedMetrics | BinnedPsiFeatureMetrics | BinnedSpcFeatureMetrics
        """

    def register_profile(self, profile: Any, set_active: bool = False) -> bool:
        """Registers a drift profile with the server

        Args:
            profile:
                Drift profile
            set_active:
                Whether to set the profile as active or inactive

        Returns:
            boolean
        """

    def update_profile_status(self, request: ProfileStatusRequest) -> bool:
        """Update profile status

        Args:
            request:
                ProfileStatusRequest

        Returns:
            boolean
        """

    def get_alerts(self, request: DriftAlertRequest) -> List[Alert]:
        """Get alerts

        Args:
            request:
                DriftAlertRequest

        Returns:
            List[Alert]
        """

    def download_profile(self, request: GetProfileRequest, path: Optional[Path]) -> str:
        """Download profile

        Args:
            request:
                GetProfileRequest
            path:
                Path to save profile

        Returns:
            Path to downloaded profile
        """

    def get_paginated_traces(self, request: Any) -> Any: ...

class BinnedMetricStats:
    avg: float
    lower_bound: float
    upper_bound: float

    def __str__(self) -> str: ...

class BinnedMetric:
    metric: str
    created_at: List[datetime.datetime]
    stats: List[BinnedMetricStats]

    def __str__(self) -> str: ...

class BinnedMetrics:
    @property
    def metrics(self) -> Dict[str, BinnedMetric]: ...
    def __str__(self) -> str: ...
    def __getitem__(self, key: str) -> Optional[BinnedMetric]: ...

class BinnedPsiMetric:
    created_at: List[datetime.datetime]
    psi: List[float]
    overall_psi: float
    bins: Dict[int, float]

    def __str__(self) -> str: ...

class BinnedPsiFeatureMetrics:
    features: Dict[str, BinnedMetric]

    def __str__(self) -> str: ...

class SpcDriftFeature:
    created_at: List[datetime.datetime]
    values: List[float]

    def __str__(self) -> str: ...

class BinnedSpcFeatureMetrics:
    features: Dict[str, SpcDriftFeature]

    def __str__(self) -> str: ...
