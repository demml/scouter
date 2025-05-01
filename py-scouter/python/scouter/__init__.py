# type: ignore
# pylint: disable=no-name-in-module

from .scouter import (  # noqa
    alert,
    client,
    drift,
    logging,
    observe,
    profile,
    queue,
    test,
    types,
)
from .version import __version__

# Drift imports
Drifter = drift.Drifter
SpcDriftConfig = drift.SpcDriftConfig
SpcDriftProfile = drift.SpcDriftProfile
PsiDriftConfig = drift.PsiDriftConfig
PsiDriftProfile = drift.PsiDriftProfile
CustomMetric = drift.CustomMetric
CustomDriftProfile = drift.CustomDriftProfile
CustomMetricDriftConfig = drift.CustomMetricDriftConfig

# Profile imports
DataProfiler = profile.DataProfiler
DataProfile = profile.DataProfile

# Queue imports
ScouterQueue = queue.ScouterQueue
Queue = queue.Queue
KafkaConfig = queue.KafkaConfig
RabbitMQConfig = queue.RabbitMQConfig
RedisConfig = queue.RedisConfig
Feature = queue.Feature
Features = queue.Features
Metric = queue.Metric
Metrics = queue.Metrics

# Type imports
CommonCrons = types.CommonCrons

# Alert imports
PsiAlertConfig = alert.PsiAlertConfig
SpcAlertConfig = alert.SpcAlertConfig
CustomMetricAlertConfig = alert.CustomMetricAlertConfig

# Client
HTTPConfig = client.HTTPConfig
ScouterClient = client.ScouterClient


__all__ = [
    "__version__",
    # Drift
    "Drifter",
    "SpcDriftConfig",
    "SpcDriftProfile",
    "PsiDriftConfig",
    "PsiDriftProfile",
    "CustomMetric",
    "CustomDriftProfile",
    "CustomMetricDriftConfig",
    # Profile
    "DataProfiler",
    "DataProfile",
    # Queue
    "ScouterQueue",
    "Queue",
    "KafkaConfig",
    "RabbitMQConfig",
    "RedisConfig",
    "Feature",
    "Features",
    "Metric",
    "Metrics",
    # Type
    "CommonCrons",
    # Alert
    "PsiAlertConfig",
    "SpcAlertConfig",
    "CustomMetricAlertConfig",
    # Client
    "HTTPConfig",
    "ScouterClient",
]
