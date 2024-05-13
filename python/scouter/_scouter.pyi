from numpy.typing import NDArray
from typing import List, Optional, Dict

class FeatureMonitorProfile:
    @property
    def id(self) -> str:
        """Return the id."""
        ...
    @property
    def center(self) -> float:
        """Return the center."""
        ...
    @property
    def lcl(self) -> float:
        """Return the lcl."""
        ...
    @property
    def ucl(self) -> float:
        """Return the ucl."""
        ...
    @property
    def timestamp(self) -> str:
        """Return the timestamp."""
        ...

class MonitorProfile:
    @property
    def features(self) -> Dict[str, FeatureMonitorProfile]:
        """Return the list of features."""
        ...

class RustScouter:
    def __init__(self) -> None:
        """Create a data profiler object."""
        ...
    def create_data_profile_f32(
        self,
        features: List[str],
        array: NDArray,
    ) -> None:
        """Create a data profile from a f32 numpy array.

        Args:
            features:
                List of feature names.
            array:
                Numpy array to profile.

        Returns:
            Monitoring profile.
        """
        ...
    def create_data_profile_f64(
        self,
        features: List[str],
        array: NDArray,
    ) -> None:
        """Create a data profile from a f32 numpy array.

        Args:
            features:
                List of feature names.
            array:
                Numpy array to profile.

        Returns:
            Monitoring profile.
        """
        ...
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
        ...
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
        ...
