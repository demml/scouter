# pylint: disable=no-name-in-module
# type: ignore

from .scouter import (  # noqa
    alert,
    client,
    drift,
    logging,
    observe,
    profile,
    queue,
    types,
    test,
)
from .version import __version__

Drifter = drift.Drifter
DataProfiler = profile.DataProfiler

__all__ = ["__version__", "Drifter", "DataProfiler"]
