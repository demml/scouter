---
title: "Python API reference"
description: "Top-level Python exports available from the scouter package."
---

This page covers the public names exported from `scouter`.
For end-to-end setup and examples, start with the guides in the rest of the docs.

## Package modules

- `alert`: Alert configuration objects for Slack, OpsGenie, and console dispatch.
- `bifrost`: Data access helpers for table-oriented reads and writes.
- `client`: HTTP client helpers for registering profiles and querying results.
- `drift`: Drift profile creation and drift result types.
- `evaluate`: Offline and online evaluation helpers, task types, and result handling.
- `agent`: Provider integrations, prompt types, and LLM-facing helpers.
- `logging`: Rust-backed logging configuration and logger access.
- `mock`: Fixtures and mock utilities used in tests and examples.
- `observe`: Observation helpers for runtime instrumentation.
- `profile`: Data profiling classes and feature statistics.
- `queue`: Queue producers and async ingestion helpers.
- `trace`: Convenience wrappers for trace context utilities.
- `tracing`: Tracing runtime, exporters, and instrumentors.
- `transport`: Transport configuration for HTTP, gRPC, Kafka, RabbitMQ, and Redis.
- `types`: Shared feature, metric, and alert types.
- `util`: Utility helpers used by client-side integrations.
- `service_map`: Service map middleware and connection records.

## Top-level exports

### Drift and profiling

#### `Drifter`

```python
Drifter()
```

Instantiate Rust Drifter class that is used to create monitoring profiles and compute drifts.

#### `SpcDriftConfig`

```python
SpcDriftConfig(space: str = '__missing__', name: str = '__missing__', version: str = '0.1.0', sample_size: int = 25, alert_config: SpcAlertConfig = SpcAlertConfig(), config_path: Optional[Path] = None)
```

Initialize monitor config

#### `SpcDriftProfile`

```python
class SpcDriftProfile
```

See the signature above and the guide docs for usage examples.

#### `SpcFeatureDriftProfile`

```python
class SpcFeatureDriftProfile
```

See the signature above and the guide docs for usage examples.

#### `SpcFeatureDrift`

```python
class SpcFeatureDrift
```

See the signature above and the guide docs for usage examples.

#### `SpcDriftMap`

```python
class SpcDriftMap
```

Drift map of features

#### `PsiDriftConfig`

```python
PsiDriftConfig(space: str = '__missing__', name: str = '__missing__', version: str = '0.1.0', alert_config: PsiAlertConfig = PsiAlertConfig(), config_path: Optional[Path] = None, categorical_features: Optional[list[str]] = None, binning_strategy: QuantileBinning | EqualWidthBinning = QuantileBinning(num_bins=10))
```

Initialize monitor config

#### `PsiDriftProfile`

```python
class PsiDriftProfile
```

See the signature above and the guide docs for usage examples.

#### `PsiDriftMap`

```python
class PsiDriftMap
```

Drift map of features

#### `FeatureMap`

```python
class FeatureMap
```

See the signature above and the guide docs for usage examples.

#### `CustomMetric`

```python
CustomMetric(name: str, baseline_value: float, alert_threshold: AlertThreshold, delta: Optional[float] = None)
```

Initialize a custom metric for alerting.

#### `CustomDriftProfile`

```python
CustomDriftProfile(config: CustomMetricDriftConfig, metrics: list[CustomMetric])
```

Initialize a CustomDriftProfile instance.

#### `CustomMetricDriftConfig`

```python
CustomMetricDriftConfig(space: str = '__missing__', name: str = '__missing__', version: str = '0.1.0', sample_size: int = 25, alert_config: CustomMetricAlertConfig = CustomMetricAlertConfig())
```

Initialize drift config Args: space: Model space name: Model name version: Model version. Defaults to 0.1.0 sample_size: Sample size alert_config: Custom metric alert configuration

#### `QuantileBinning`

```python
QuantileBinning(num_bins: int = 10)
```

Initialize the quantile binning strategy.

#### `EqualWidthBinning`

```python
EqualWidthBinning(method: EqualWidthMethods = Doane())
```

Initialize the equal-width binning configuration.

#### `Manual`

```python
Manual(num_bins: int)
```

Manual equal-width binning strategy.

#### `SquareRoot`

```python
SquareRoot()
```

Use the SquareRoot equal-width method.

#### `Sturges`

```python
Sturges()
```

Use the Sturges equal-width method.

#### `Rice`

```python
Rice()
```

Use the Rice equal-width method.

#### `Doane`

```python
Doane()
```

Use the Doane equal-width method.

#### `Scott`

```python
Scott()
```

Use the Scott equal-width method.

#### `TerrellScott`

```python
TerrellScott()
```

Use the Terrell-Scott equal-width method.

#### `FreedmanDiaconis`

```python
FreedmanDiaconis()
```

Use the Freedman–Diaconis equal-width method.

#### `DataProfiler`

```python
DataProfiler()
```

Instantiate DataProfiler class that is used to profile data

#### `DataProfile`

```python
class DataProfile
```

Data profile of features

### Queue and transport

#### `ScouterQueue`

```python
class ScouterQueue
```

Main queue class for Scouter. Publishes drift records to the configured transport

#### `Queue`

```python
class Queue
```

Individual queue associated with a drift profile

#### `ScouterClient`

```python
ScouterClient(config: Optional[HttpConfig] = None)
```

Helper client for interacting with Scouter Server

#### `DatasetClient`

```python
DatasetClient(transport: Any, table_config: Optional[TableConfig] = None)
```

Dataset client for reading and querying datasets.

#### `DatasetProducer`

```python
DatasetProducer(table_config: TableConfig, transport: Any, write_config: Optional[WriteConfig] = None)
```

Real-time streaming producer for the Scouter dataset engine.

#### `TableConfig`

```python
TableConfig(model: Type[Any], catalog: str, schema_name: str, table: str, partition_columns: Optional[List[str]] = None)
```

Configuration for a dataset table, derived from a Pydantic model.

#### `WriteConfig`

```python
WriteConfig(batch_size: int = 1000, scheduled_delay_secs: int = 30)
```

Configuration for dataset write behavior.

#### `HttpConfig`

```python
HttpConfig(server_uri: Optional[str] = None, username: Optional[str] = None, password: Optional[str] = None, auth_token: Optional[str] = None)
```

HTTP configuration to use with the HTTPProducer.

#### `GrpcConfig`

```python
GrpcConfig(server_uri: Optional[str] = None, username: Optional[str] = None, password: Optional[str] = None, timeout_secs: Optional[int] = None, connect_timeout_secs: Optional[int] = None, keep_alive_interval_secs: Optional[int] = None, keep_alive_timeout_secs: Optional[int] = None, keep_alive_while_idle: Optional[bool] = None)
```

gRPC configuration to use with the GrpcProducer.

#### `KafkaConfig`

```python
KafkaConfig(username: Optional[str] = None, password: Optional[str] = None, brokers: Optional[str] = None, topic: Optional[str] = None, compression_type: Optional[str] = None, message_timeout_ms: int = 600000, message_max_bytes: int = 2097164, log_level: LogLevel = LogLevel.Info, config: Dict[str, str] = {}, max_retries: int = 3)
```

Kafka configuration for connecting to and publishing messages to Kafka brokers.

#### `RabbitMQConfig`

```python
RabbitMQConfig(host: Optional[str] = None, port: Optional[int] = None, username: Optional[str] = None, password: Optional[str] = None, queue: Optional[str] = None, max_retries: int = 3)
```

RabbitMQ configuration to use with the RabbitMQProducer.

#### `RedisConfig`

```python
RedisConfig(address: Optional[str] = None, chanel: Optional[str] = None)
```

Redis configuration to use with a Redis producer

### Evaluation

#### `AgentEvalConfig`

```python
AgentEvalConfig(space: str = '__missing__', name: str = '__missing__', version: str = '0.1.0', sample_ratio: float = 1.0, alert_config: AgentAlertConfig = AgentAlertConfig())
```

Initialize drift config Args: space: Space to associate with the config name: Name to associate with the config version: Version to associate with the config. Defaults to 0.1.0 sample_ratio: Sample rate percentage for data collection. Must be between 0.0 and 1.0. Defaults to 1.0 (100%). alert_config: Custom metric alert configuration

#### `AgentEvalProfile`

```python
AgentEvalProfile(tasks: _TASK_TYPES, config: Optional[AgentEvalConfig] = None, alias: Optional[str] = None)
```

Profile for LLM evaluation and drift detection.

#### `EvalRecord`

```python
EvalRecord(context: Optional[Context] = None, record_id: Optional[str] = None, *, session_id: Optional[str] = None, media: Optional[List[Union[EvalMedia, ImageMedia, DocumentMedia]]] = None, profile_uid: Optional[str] = None, tags: Optional[List[str]] = None, trace_id: Optional[str] = None)
```

LLM record containing context tied to a Large Language Model interaction that is used to evaluate drift in LLM responses.

#### `LLMJudgeTask`

```python
LLMJudgeTask(id: str, prompt: Prompt[Any], expected_value: Any, context_path: Optional[str], operator: ComparisonOperator, description: Optional[str] = None, depends_on: Optional[List[str]] = None, max_retries: Optional[int] = None, condition: bool = False)
```

LLM-powered evaluation task for complex assessments.

#### `AssertionTask`

```python
AssertionTask(id: str, expected_value: Any, operator: ComparisonOperator, context_path: Optional[str] = None, item_context_path: Optional[str] = None, description: Optional[str] = None, depends_on: Optional[Sequence[str]] = None, condition: bool = False)
```

Assertion-based evaluation task for LLM monitoring.

#### `ComparisonOperator`

```python
class ComparisonOperator
```

Comparison operators for assertion-based evaluations.

#### `EvalResults`

```python
class EvalResults
```

Defines the results of an LLM eval metric

#### `TraceAssertion`

```python
class TraceAssertion
```

Assertion target for trace and span properties.

#### `TraceAssertionTask`

```python
TraceAssertionTask(id: str, assertion: TraceAssertion, expected_value: Any, operator: ComparisonOperator, description: Optional[str] = None, depends_on: Optional[List[str]] = None, condition: bool = False)
```

Trace-based evaluation task for behavioral assertions.

#### `SpanStatus`

```python
class SpanStatus
```

Status codes for trace spans.

#### `AggregationType`

```python
class AggregationType
```

Aggregation operations for span attribute values.

#### `SpanFilter`

```python
class SpanFilter
```

Filter for selecting specific spans within a trace.

### Shared types and scheduling

#### `Feature`

```python
Feature
```

No stub docstring was found for this export.

#### `Features`

```python
Features(features: List[QueueFeature] | Dict[str, Union[int, float, str]])
```

Initialize a features class

#### `Metric`

```python
Metric(name: str, value: float | int)
```

Initialize metric

#### `Metrics`

```python
Metrics(metrics: List[Metric] | Dict[str, Union[int, float]])
```

Initialize metrics

#### `CommonCrons`

```python
class CommonCrons
```

See the signature above and the guide docs for usage examples.

#### `PsiAlertConfig`

```python
PsiAlertConfig(dispatch_config: Optional[SlackDispatchConfig | OpsGenieDispatchConfig] = None, schedule: Optional[str | CommonCrons] = None, features_to_monitor: List[str] = [], threshold: Optional[PsiThresholdType] = PsiChiSquareThreshold())
```

Initialize alert config

#### `SpcAlertConfig`

```python
SpcAlertConfig(rule: Optional[SpcAlertRule] = None, dispatch_config: Optional[SlackDispatchConfig | OpsGenieDispatchConfig] = None, schedule: Optional[str | CommonCrons] = None, features_to_monitor: List[str] = [])
```

Initialize alert config

#### `CustomMetricAlertConfig`

```python
CustomMetricAlertConfig(dispatch_config: Optional[SlackDispatchConfig | OpsGenieDispatchConfig] = None, schedule: Optional[str | CommonCrons] = None)
```

Initialize alert config

#### `Bifrost`

```python
Bifrost(table_config: TableConfig, transport: Any, write_config: Optional[WriteConfig] = None)
```

Unified read/write client for the Bifrost dataset engine.

### Service map

#### `ServiceConnectionRecord`

```python
ServiceConnectionRecord
```

No stub docstring was found for this export.

#### `ServiceMapMiddleware`

```python
ServiceMapMiddleware
```

No stub docstring was found for this export.

### Other exports

#### `ScouterEnv`

```python
class ScouterEnv
```

See the signature above and the guide docs for usage examples.
