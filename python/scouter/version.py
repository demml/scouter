from importlib.metadata import version, PackageNotFoundError


try:
    __version__ = version("scouter")
except PackageNotFoundError:
    __version__ = "unknown"
