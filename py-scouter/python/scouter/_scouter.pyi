# pylint: disable=invalid-name

import datetime
from pathlib import Path
from typing import Dict, List, Optional, Tuple, Union

from numpy.typing import NDArray

class DriftServerRecord:
    def __init__(
        self,
        name: str,
        repository: str,
        version: str,
        feature: str,
        value: float,
    ):
        """Initialize drift server record

        Args:
            name:
                Model name
            repository:
                Model repository
            version:
                Model version
            feature:
                Feature name
            value:
                Feature value
        """
    @property
    def created_at(self) -> datetime.datetime:
        """Return the created at timestamp."""
    @property
    def name(self) -> str:
        """Return the name."""
    @property
    def repository(self) -> str:
        """Return the repository."""
    @property
    def version(self) -> str:
        """Return the version."""
    @property
    def feature(self) -> str:
        """Return the feature."""
    @property
    def value(self) -> float:
        """Return the sample value."""
    def __str__(self) -> str:
        """Return the string representation of the record."""
    def model_dump_json(self) -> str:
        """Return the json representation of the record."""
    def to_dict(self) -> Dict[str, str]:
        """Return the dictionary representation of the record."""

class Every30Minutes:
    def __init__(self) -> None:
        """Initialize the cron schedule"""
    @property
    def cron(self) -> str:
        """Return the cron schedule"""

class EveryHour:
    def __init__(self) -> None:
        """Initialize the cron schedule"""
    @property
    def cron(self) -> str:
        """Return the cron schedule"""

class Every6Hours:
    def __init__(self) -> None:
        """Initialize the cron schedule"""
    @property
    def cron(self) -> str:
        """Return the cron schedule"""

class Every12Hours:
    def __init__(self) -> None:
        """Initialize the cron schedule"""
    @property
    def cron(self) -> str:
        """Return the cron schedule"""

class EveryDay:
    def __init__(self) -> None:
        """Initialize the cron schedule"""
    @property
    def cron(self) -> str:
        """Return the cron schedule"""

class EveryWeek:
    def __init__(self) -> None:
        """Initialize the cron schedule"""
    @property
    def cron(self) -> str:
        """Return the cron schedule"""

class CommonCron:
    def __init__(self) -> None:
        """Initialize the common cron class from rust"""
    @property
    def EVERY_30_MINUTES(self) -> str:
        """Every 30 minutes cron schedule"""
    @property
    def EVERY_HOUR(self) -> str:
        """Every hour cron schedule"""
    @property
    def EVERY_6_HOURS(self) -> str:
        """Every 6 hours cron schedule"""
    @property
    def EVERY_12_HOURS(self) -> str:
        """Every 12 hours cron schedule"""
    @property
    def EVERY_DAY(self) -> str:
        """Every day cron schedule"""
    @property
    def EVERY_WEEK(self) -> str:
        """Every week cron schedule"""

class PercentageAlertRule:
    def __init__(self, rule: Optional[float] = None) -> None:
        """Initialize alert rule

        Args:
            rule:
                Rule to use for percentage alerting (float)
        """
    @property
    def rule(self) -> float:
        """Return the alert rule"""

class ProcessAlertRule:
    def __init__(self, rule: Optional[str] = None) -> None:
        """Initialize alert rule

        Args:
            rule:
                Rule to use for alerting. Eight digit integer string.
                Defaults to '8 16 4 8 2 4 1 1'
        """
    @property
    def rule(self) -> str:
        """Return the alert rule"""

class AlertRule:
    def __init__(
        self,
        percentage_rule: Optional[PercentageAlertRule] = None,
        process_rule: Optional[ProcessAlertRule] = None,
    ) -> None:
        """Initialize alert rule

        Args:
            rule:
                Rule to use for alerting.
        """
    @property
    def process(self) -> Optional[ProcessAlertRule]:
        """Return the control alert rule"""
    @property
    def percentage(self) -> Optional[PercentageAlertRule]:
        """Return the percentage alert rule"""

class Alert:
    def __init__(self, alert_type: str, zone: str):
        """Initialize alert"""
    @property
    def kind(self) -> str:
        """Alert kind"""
    @property
    def zone(self) -> str:
        """Zone associated with alert"""

class FeatureAlert:
    @property
    def feature(self) -> str:
        """Return the feature."""
    @property
    def alerts(self) -> List[Alert]:
        """Return the alerts."""
    @property
    def indices(self) -> Dict[Union[str, int], List[List[int]]]:
        """Return the alert indices"""

class FeatureAlerts:
    @property
    def features(self) -> Dict[str, FeatureAlert]:
        """Return the feature alerts."""

class FeatureDriftProfile:
    @property
    def id(self) -> str:
        """Return the id."""
    @property
    def center(self) -> float:
        """Return the center."""
    @property
    def one_ucl(self) -> float:
        """Return the zone 1 ucl."""
    @property
    def one_lcl(self) -> float:
        """Return the zone 1 lcl."""
    @property
    def two_ucl(self) -> float:
        """Return the zone 2 ucl."""
    @property
    def two_lcl(self) -> float:
        """Return the zone 2 lcl."""
    @property
    def three_ucl(self) -> float:
        """Return the zone 3 ucl."""
    @property
    def three_lcl(self) -> float:
        """Return the zone 3 lcl."""
    @property
    def timestamp(self) -> str:
        """Return the timestamp."""

class DriftConfig:
    def __init__(
        self,
        name: str,
        repository: str,
        version: str = "0.1.0",
        sample: bool = True,
        sample_size: int = 25,
        schedule: str = "0 0 0 * * *",
        alert_rule: AlertRule = AlertRule(),
    ):
        """Initialize monitor config

        Args:
            name:
                Model name
            repository:
                Model repository
            version:
                Model version. Defaults to 0.1.0
            sample:
                Whether to sample or not
            sample_size:
                Sample size
            schedule:
                Schedule to run monitor. Defaults to daily at midnight
            alert_rule:
                Alert rule to use. Defaults to Standard
        """
    @property
    def sample_size(self) -> int:
        """Return the sample size."""
    @property
    def sample(self) -> bool:
        """Whether to sample or not"""
    @property
    def name(self) -> str:
        """Model Name"""
    @property
    def repository(self) -> str:
        """Model repository"""
    @property
    def version(self) -> str:
        """Model version"""
    @property
    def schedule(self) -> str:
        """Schedule to run monitor"""
    @property
    def alert_rule(self) -> AlertRule:
        """Alert rule to use"""

class DriftProfile:
    @property
    def features(self) -> Dict[str, FeatureDriftProfile]:
        """Return the list of features."""
    @property
    def config(self) -> DriftConfig:
        """Return the monitor config."""
    def __str__(self) -> str:
        """Sting representation of DriftProfile"""

class Distinct:
    @property
    def count(self) -> int:
        """total unqiue value counts"""
    @property
    def percent(self) -> float:
        """percent value uniqueness"""

class Quantiles:
    @property
    def q25(self) -> float:
        """25th quantile"""
    @property
    def q50(self) -> float:
        """50th quantile"""
    @property
    def q75(self) -> float:
        """75th quantile"""
    @property
    def q99(self) -> float:
        """99th quantile"""

class Histogram:
    @property
    def bins(self) -> List[float]:
        """Bin values"""
    @property
    def bin_counts(self) -> List[int]:
        """Bin counts"""

class FeatureDataProfile:
    @property
    def id(self) -> str:
        """Return the id."""
    @property
    def mean(self) -> float:
        """Return the mean."""
    @property
    def stddev(self) -> float:
        """Return the stddev."""
    @property
    def min(self) -> float:
        """Return the min."""
    @property
    def max(self) -> float:
        """Return the max."""
    @property
    def timestamp(self) -> datetime.datetime:
        """Return the timestamp."""
    @property
    def distinct(self) -> Distinct:
        """Distinct value counts"""
    @property
    def quantiles(self) -> Quantiles:
        """Value quantiles"""
    @property
    def histogram(self) -> Histogram:
        """Value histograms"""

class DataProfile:
    """Data profile of features"""

    @property
    def features(self) -> Dict[str, FeatureDataProfile]:
        """Returns dictionary of features and their data profiles"""
    def __str__(self) -> str:
        """Return string representation of the data profile"""
    def model_dump_json(self) -> str:
        """Return json representation of data profile"""
    @staticmethod
    def load_from_json(model: str) -> "DataProfile":
        """Load drift map from json"""
    def save_to_json(self, path: Optional[Path] = None) -> None:
        """Save data profile to json file

        Args:
            path:
                Optional path to save the data profile. If None, outputs to "data_profile.json.

        """

class FeatureDrift:
    @property
    def samples(self) -> List[float]:
        """Return list of samples"""
    @property
    def drift(self) -> List[float]:
        """Return list of drift values"""
    def __str__(self) -> str:
        """Return string representation of feature drift"""

class DriftMap:
    """Drift map of features"""

    def __init__(self, service_name: Optional[str]) -> None:
        """Initialize data profile

        Args:
            service_name:
                Optional name of service associated with drift map
        """
    @property
    def name(self) -> str:
        """name to associate with drift map"""
    @property
    def repository(self) -> str:
        """Repository to associate with drift map"""
    @property
    def version(self) -> str:
        """Version to associate with drift map"""
    @property
    def features(self) -> Dict[str, FeatureDrift]:
        """Returns dictionary of features and their data profiles"""
    def __str__(self) -> str:
        """Return string representation of data drift"""
    def model_dump_json(self) -> str:
        """Return json representation of data drift"""
    @staticmethod
    def load_from_json(model: str) -> "DriftMap":
        """Load drift map from json"""
    def save_to_json(self, path: Optional[Path] = None) -> None:
        """Save drift map to json file

        Args:
            path:
                Optional path to save the drift map. If None, outputs to "drift_map.json.

        """
    def to_numpy(self) -> Tuple[NDArray, List[str]]:
        """Return drift map as a numpy array and list of features"""
    def to_service_record(self) -> List[DriftServerRecord]:
        """Return drift map as a drift server record"""

class ScouterProfiler:
    def __init__(self) -> None:
        """Instantiate Rust ScouterProfiler class that is
        used to profile data"""
    def create_data_profile_f32(
        self,
        features: List[str],
        array: NDArray,
        bin_size: int,
    ) -> DataProfile:
        """Create a data profile from a f32 numpy array.

        Args:
            features:
                List of feature names.
            array:
                Numpy array to profile.
            bin_size:
                Optional bin size for histograms. Defaults to 20 bins.

        Returns:
            Monitoring profile.
        """
    def create_data_profile_f64(
        self,
        features: List[str],
        array: NDArray,
        bin_size: int,
    ) -> DataProfile:
        """Create a data profile from a f32 numpy array.

        Args:
            features:
                List of feature names.
            array:
                Numpy array to profile.
            bin_size:
                Optional bin size for histograms. Defaults to 20 bins.

        Returns:
            Monitoring profile.
        """

class ScouterDrifter:
    def __init__(self) -> None:
        """Instantiate Rust ScouterMonitor class that is
        used to create monitoring profiles and compute drifts.
        """
    def create_drift_profile_f32(
        self,
        features: List[str],
        array: NDArray,
        monitor_config: DriftConfig,
    ) -> DriftProfile:
        """Create a monitoring profile from a f64 numpy array.

        Args:
            features:
                List of feature names.
            array:
                Numpy array to profile.
            monitor_config:
                Monitor config.

        Returns:
            Monitoring profile.
        """
    def create_drift_profile_f64(
        self,
        features: List[str],
        array: NDArray,
        monitor_config: DriftConfig,
    ) -> DriftProfile:
        """Create a monitoring profile from a f64 numpy array.

        Args:
            features:
                List of feature names.
            array:
                Numpy array to profile.
            monitor_config:
                monitor config.

        Returns:
            Monitoring profile.
        """
    def compute_drift_f32(
        self,
        features: List[str],
        array: NDArray,
        drift_profile: DriftProfile,
    ) -> DriftMap:
        """Compute drift from a f32 numpy array.

        Args:
            features:
                List of feature names.
            array:
                Numpy array to profile.
            drift_profile:
                Monitoring profile.


        Returns:
            DriftMap
        """
    def compute_drift_f64(
        self,
        features: List[str],
        array: NDArray,
        drift_profile: DriftProfile,
    ) -> DriftMap:
        """Compute drift from a f64 numpy array.

        Args:
            features:
                List of feature names.
            array:
                Numpy array to profile.
            drift_profile:
                Monitoring profile.


        Returns:
            DriftMap.
        """
    def generate_alerts(
        self,
        drift_array: NDArray,
        features: List[str],
        alert_rule: AlertRule,
    ) -> FeatureAlerts:
        """Generate alerts from a drift array and feature list

        Args:
            drift_array:
                Numpy array of drift values.
            features:
                List of feature names. Must match drift array.
            alert_rule:
                Alert rule to use.

        Returns:
            List of alerts.
        """
    def sample_data_f32(
        self,
        features: List[str],
        array: NDArray,
        drift_profile: DriftProfile,
    ) -> List[DriftServerRecord]:
        """Sample data from a f32 numpy array.

        Args:
            features:
                List of feature names.
            array:
                Numpy array to profile.
            drift_profile:
                Monitoring profile.

        Returns:
            List of server records
        """
    def sample_data_f64(
        self,
        features: List[str],
        array: NDArray,
        drift_profile: DriftProfile,
    ) -> List[DriftServerRecord]:
        """Sample data from a f64 numpy array.

        Args:
            features:
                List of feature names.
            array:
                Numpy array to profile.
            drift_profile:
                Monitoring profile.

        Returns:
            List of server records
        """
