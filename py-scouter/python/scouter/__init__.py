"""Scouter: ML monitoring and drift detection library."""

# pylint: disable=no-name-in-module,import-error

from .scouter import (  # type: ignore
    alert,
    client,
    drift,
    evaluate,
    llm,
    logging,
    mock,
    observe,
    profile,
    queue,
    types,
)
from .version import __version__

# Drift imports
Drifter = drift.Drifter
SpcDriftConfig = drift.SpcDriftConfig
SpcDriftProfile = drift.SpcDriftProfile
SpcFeatureDriftProfile = drift.SpcFeatureDriftProfile
SpcFeatureDrift = drift.SpcFeatureDrift
SpcDriftMap = drift.SpcDriftMap
PsiDriftConfig = drift.PsiDriftConfig
PsiDriftProfile = drift.PsiDriftProfile
PsiDriftMap = drift.PsiDriftMap
FeatureMap = drift.FeatureMap
CustomMetric = drift.CustomMetric
CustomDriftProfile = drift.CustomDriftProfile
CustomMetricDriftConfig = drift.CustomMetricDriftConfig
LLMDriftMetric = drift.LLMDriftMetric
LLMDriftConfig = drift.LLMDriftConfig
LLMDriftProfile = drift.LLMDriftProfile


# Profile imports - direct from Rust extension
DataProfiler = profile.DataProfiler
DataProfile = profile.DataProfile

# Queue imports - direct from Rust extension
ScouterQueue = queue.ScouterQueue
Queue = queue.Queue
KafkaConfig = queue.KafkaConfig
RabbitMQConfig = queue.RabbitMQConfig
RedisConfig = queue.RedisConfig
Feature = queue.Feature
Features = queue.Features
Metric = queue.Metric
Metrics = queue.Metrics

# Type imports - direct from Rust extension
CommonCrons = types.CommonCrons

# Alert imports - direct from Rust extension
PsiAlertConfig = alert.PsiAlertConfig
SpcAlertConfig = alert.SpcAlertConfig
CustomMetricAlertConfig = alert.CustomMetricAlertConfig

# Client imports - direct from Rust extension
HTTPConfig = client.HTTPConfig
ScouterClient = client.ScouterClient

# Evaluate imports
LLMEvalMetric = evaluate.LLMEvalMetric
LLMEvalResults = evaluate.LLMEvalResults
LLMEvalRecord = evaluate.LLMEvalRecord
evaluate_llm = evaluate.evaluate_llm

__all__ = [
    "__version__",
    # Drift - from Python wrappers
    "Drifter",
    "SpcDriftConfig",
    "SpcDriftProfile",
    "PsiDriftConfig",
    "PsiDriftProfile",
    "PsiDriftMap",
    "CustomMetric",
    "CustomDriftProfile",
    "CustomMetricDriftConfig",
    "LLMDriftMetric",
    "LLMDriftConfig",
    "LLMDriftProfile",
    "FeatureMap",
    "SpcFeatureDriftProfile",
    "SpcFeatureDrift",
    "SpcDriftMap",
    # Profile - from Rust extension
    "DataProfiler",
    "DataProfile",
    # Queue - from Rust extension
    "ScouterQueue",
    "Queue",
    "KafkaConfig",
    "RabbitMQConfig",
    "RedisConfig",
    "Feature",
    "Features",
    "Metric",
    "Metrics",
    # Types - from Rust extension
    "CommonCrons",
    # Alert - from Rust extension
    "PsiAlertConfig",
    "SpcAlertConfig",
    "CustomMetricAlertConfig",
    # Client - from Rust extension
    "HTTPConfig",
    "ScouterClient",
    # Evaluate - from Python wrappers
    "LLMEvalMetric",
    "LLMEvalResults",
    "LLMEvalRecord",
    "evaluate_llm",
    # Submodules - for advanced usage
    "alert",
    "client",
    "llm",
    "logging",
    "mock",
    "observe",
    "profile",
    "queue",
    "types",
    "drift",
    "evaluate",
]
