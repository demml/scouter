from numpy.typing import NDArray
from typing import List, Optional

class Scouter:
    def __init__(
        self,
        features: Optional[List[str]] = None,
    ) -> None:
        """Create a data profiler object."""
        ...
    def create_data_profile(self, array: NDArray) -> None:
        """Create a data profile from a numpy array."""
        ...
    def create_monitoring_profile(self, array: NDArray) -> None:
        """Create a monitoring profile from a numpy array."""
        ...
