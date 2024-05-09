from numpy.typing import NDArray
from typing import List, Optional

class RustScouter:
    def __init__(
        self,
        features: Optional[List[str]] = None,
    ) -> None:
        """Create a data profiler object."""
        ...
    def create_data_profile32(self, array: NDArray) -> None:
        """Create a data profile from a f32 numpy array."""
        ...

    def create_data_profile64(self, array: NDArray) -> None:
        """Create a data profile from a f32 numpy array."""
        ...

    def create_monitoring_profile32(self, array: NDArray) -> None:
        """Create a monitoring profile from a f64 numpy array."""
        ...

    def create_monitoring_profile64(self, array: NDArray) -> None:
        """Create a monitoring profile from a f64 numpy array."""
        ...
