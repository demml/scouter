from pathlib import Path
from typing import Dict, List, Optional
from scouter.utils.types import AlertRules
import datetime
from numpy.typing import NDArray

class Alert:
    def __init__(self, alert_type: str, zone: str):
        """Initialize alert"""
    @property
    def kind(self) -> str:
        """Alert kind"""

    @property
    def zone(self) -> str:
        """Zone associated with alert"""

class FeatureMonitorProfile:
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

class MonitorConfig:
    def __init__(
        self,
        name: str,
        repository: str,
        version: str = "0.1.0",
        sample: bool = True,
        sample_size: int = 25,
        alert_rule: str = AlertRules.Standard.value,
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
    def alert_rule(self) -> str:
        """Alert rule to use"""

    def set_config(
        self,
        sample: Optional[bool],
        sample_size: Optional[int],
        name: Optional[str],
        repository: Optional[str],
        version: Optional[str],
    ) -> None:
        """Set the monitor config

        Args:
            sample:
                Whether to sample or not
            sample_size:
                Sample size
            name:
                Model name
            repository:
                Model repository
            version:
                Model version
        """

class MonitorProfile:
    @property
    def features(self) -> Dict[str, FeatureMonitorProfile]:
        """Return the list of features."""

    @property
    def config(self) -> MonitorConfig:
        """Return the monitor config."""

    def __str__(self) -> str:
        """Sting representation of MonitorProfile"""

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
    def service_name(self) -> Optional[str]:
        """Optional service name to associate with drift map"""

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

class DriftConfig:
    """Config to associate with new sample of data to compute drift on"""
    def __init__(
        self,
        features: List[str],
        monitor_profile: MonitorProfile,
        service_name: Optional[str],
    ) -> None:
        """Initialize drift config

        Args:
            features:
                List of feature names.
            monitor_profile:
                Monitoring profile.
            service_name:
                Optional service name.
        """

    @property
    def features(self) -> List[str]:
        """Features"""

    @property
    def monitor_profile(self) -> MonitorProfile:
        """Monitor profile to use when computing drift"""

    @property
    def service_name(self) -> Optional[str]:
        """Optional service name to associate with drift"""

    def __str__(self) -> str:
        """Return string representation of drift config"""

class RustScouter:
    def __init__(self) -> None:
        """Instantiate RustScouter"""

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
    def create_monitor_profile_f32(
        self,
        features: List[str],
        array: NDArray,
        monitor_config: MonitorConfig,
    ) -> MonitorProfile:
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
    def create_monitor_profile_f64(
        self,
        features: List[str],
        array: NDArray,
        monitor_config: MonitorConfig,
    ) -> MonitorProfile:
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
        monitor_profile: MonitorProfile,
    ) -> DriftMap:
        """Compute drift from a f32 numpy array.

        Args:
            features:
                List of feature names.
            array:
                Numpy array to profile.
            monitor_profile:
                Monitoring profile.


        Returns:
            DriftMap
        """
    def compute_drift_f64(
        self,
        features: List[str],
        array: NDArray,
        monitor_profile: MonitorProfile,
    ) -> DriftMap:
        """Compute drift from a f64 numpy array.

        Args:
            features:
                List of feature names.
            array:
                Numpy array to profile.
            monitor_profile:
                Monitoring profile.


        Returns:
            DriftMap.
        """
