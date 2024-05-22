from typing import List, Dict, Optional
from numpy.typing import NDArray
from pathlib import Path

class FeatureMonitorProfile:
    @property
    def id(self) -> str:
        """Return the id."""
    @property
    def center(self) -> float:
        """Return the center."""
    @property
    def lcl(self) -> float:
        """Return the lcl."""
    @property
    def ucl(self) -> float:
        """Return the ucl."""
    @property
    def timestamp(self) -> str:
        """Return the timestamp."""

class MonitorProfile:
    @property
    def features(self) -> Dict[str, FeatureMonitorProfile]:
        """Return the list of features."""

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
    def timestamp(self) -> str:
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
    def load_from_json(model: str) -> "DriftMap":
        """Load drift map from json"""

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

class RustScouter:
    def __init__(self, bin_size: Optional[int]) -> None:
        """Create a data profiler object.

        Args:
            bin_size:
                Optional bin size for histograms.
        """
    def create_data_profile_f32(
        self,
        features: List[str],
        array: NDArray,
    ) -> DataProfile:
        """Create a data profile from a f32 numpy array.

        Args:
            features:
                List of feature names.
            array:
                Numpy array to profile.

        Returns:
            Monitoring profile.
        """
    def create_data_profile_f64(
        self,
        features: List[str],
        array: NDArray,
    ) -> DataProfile:
        """Create a data profile from a f32 numpy array.

        Args:
            features:
                List of feature names.
            array:
                Numpy array to profile.

        Returns:
            Monitoring profile.
        """
    def create_monitor_profile_f32(
        self,
        features: List[str],
        array: NDArray,
    ) -> MonitorProfile:
        """Create a monitoring profile from a f64 numpy array.

        Args:
            features:
                List of feature names.
            array:
                Numpy array to profile.

        Returns:
            Monitoring profile.
        """
    def create_monitor_profile_f64(
        self,
        features: List[str],
        array: NDArray,
    ) -> MonitorProfile:
        """Create a monitoring profile from a f64 numpy array.

        Args:
            features:
                List of feature names.
            array:
                Numpy array to profile.

        Returns:
            Monitoring profile.
        """
    def compute_drift_f32(
        self,
        features: List[str],
        array: NDArray,
        monitor_profile: MonitorProfile,
        sample: bool,
    ) -> DriftMap:
        """Compute drift from a f32 numpy array.

        Args:
            features:
                List of feature names.
            array:
                Numpy array to profile.
            monitor_profile:
                Monitoring profile.
            sample:
                Whether to sample the data.


        Returns:
            Monitoring profile.
        """
    def compute_drift_f64(
        self,
        features: List[str],
        array: NDArray,
        monitor_profile: MonitorProfile,
        sample: bool,
    ) -> DriftMap:
        """Compute drift from a f64 numpy array.

        Args:
            features:
                List of feature names.
            array:
                Numpy array to profile.
            monitor_profile:
                Monitoring profile.
            sample:
                Whether to sample the data.

        Returns:
            Monitoring profile.
        """
