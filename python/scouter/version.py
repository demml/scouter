from importlib.metadata import PackageNotFoundError, version

try:
    __version__ = version("scouter")
except PackageNotFoundError:
    __version__ = "unknown"
