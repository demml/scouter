# Online GenAI Evaluation with Scouter

Evaluate your GenAI services in real-time using the same evaluation tasks as offline testing. Online evaluation runs asynchronously on the Scouter server, providing continuous monitoring and observability without impacting your application's performance.

## Overview

Online GenAI evaluation enables:

- **Real-time Monitoring**: Track service quality as traffic flows through your system
- **Automated Alerting**: Get notified when metrics fall below acceptable thresholds
- **Zero Latency Impact**: Evaluations run server-side without blocking your application
- **Cost Control**: Configure sampling to balance monitoring coverage with evaluation costs (integrates with Scouter Tracing)
- **Consistent Evaluation**: Use identical tasks from offline testing for production monitoring

## What is a GenAI Drift Profile?

A **GenAI Drift Profile** combines 2 components for online-evaluation:

- **GenAIDriftConfig**: Service metadata and alert configuration
- **Evaluation Tasks**: `LLMJudgeTask` and `AssertionTask` tasks to validate service context (same as offline)

The profile executes your evaluation tasks asynchronously on sampled traffic, storing results and checking alert conditions on a configured schedule.

## Creating a GenAI Drift Profile

### 1. Define Evaluation Tasks

Use the same `LLMJudgeTask` and `AssertionTask` patterns from offline evaluation. Tasks support dependencies and conditional logic.

**Simple LLM Judge:**

```python
from scouter.evaluate import LLMJudgeTask, ComparisonOperator
from scouter.genai import Prompt, Provider, Score

relevance_prompt = Prompt(
    messages=(
        "Rate the relevance of this response to the user's query on a scale of 1-5.\n\n"
        "Query: ${user_query}\n"
        "Response: ${response}\n\n"
        "Provide your evaluation as JSON with 'score' and 'reason' fields."
    ),
    model="gpt-4o-mini",
    provider=Provider.OpenAI,
    output_type=Score
)

relevance_task = LLMJudgeTask(
    id="relevance_check",
    prompt=relevance_prompt,
    expected_value=4,
    field_path="score",
    operator=ComparisonOperator.GreaterThanOrEqual,
    description="Ensure relevance score >= 4"
)
```

**Hybrid Evaluation:**

```python
from scouter.evaluate import AssertionTask

# Fast assertion check
length_check = AssertionTask(
    id="response_not_empty",
    field_path="response",
    operator=ComparisonOperator.HasLengthGreaterThan,
    expected_value=10,
    description="Response must have meaningful length"
    condition=True
)

# Quality check only if length passes
quality_prompt = Prompt(
    messages=(
        "Rate the overall quality of this response on a scale of 1-5.\n\n"
        "Response: ${response}\n\n"
        "Consider clarity, completeness, and helpfulness."
    ),
    model="claude-3-5-sonnet-20241022",
    provider=Provider.Anthropic,
    output_type=Score
)

quality_task = LLMJudgeTask(
    id="quality_check",
    prompt=quality_prompt,
    expected_value=4,
    field_path="score",
    operator=ComparisonOperator.GreaterThanOrEqual,
    depends_on=["response_not_empty"],
    description="Quality must be >= 4"
)

tasks = [length_check, quality_task]
```

**Multi-Stage Dependent Evaluation:**

```python
# Stage 1: Category classification
category_prompt = Prompt(
    messages=(
        "Classify this query into one category: technical, sales, or support.\n\n"
        "Query: ${user_query}\n\n"
        "Return JSON: {\"category\": \"<category>\", \"confidence\": <0-1>}"
    ),
    model="gemini-2.5-flash-lite",
    provider=Provider.Google,
    output_type=CategoryResult
)

category_task = LLMJudgeTask(
    id="category_classification",
    prompt=category_prompt,
    expected_value=None,
    field_path="category",
    operator=ComparisonOperator.IsNotEmpty,
    description="Classify query category"
)

# Stage 2: Technical accuracy (only for technical queries)
technical_check = AssertionTask(
    id="is_technical",
    field_path="category_classification.category",
    operator=ComparisonOperator.Equals,
    expected_value="technical",
    depends_on=["category_classification"],
    condition=True
)

technical_quality_task = LLMJudgeTask(
    id="technical_quality",
    prompt=technical_quality_prompt,
    expected_value=4,
    field_path="score",
    operator=ComparisonOperator.GreaterThanOrEqual,
    depends_on=["is_technical"],
    description="Technical accuracy >= 4"
)

tasks = [category_task, technical_check, technical_quality_task]
```

### 2. Configure Alert Conditions

Define when alerts should trigger using `AlertCondition`. Alerts are triggerred based on the overall pass rate of your evaluation tasks. Meaning, if you have multiple tasks, the alert condition evaluates the aggregated workflow results.

```python
from scouter import AlertThreshold, AlertCondition

# Alert if average score falls below 4
alert_condition = AlertCondition(
    baseline_value=.80,  # 80% pass rate
    alert_threshold=AlertThreshold.Below,
    delta=0.05  # Alert if value < 0.75 (0.80 - 0.05), optional
)
```

**Alert Threshold Options:**

| Threshold | Behavior |
|-----------|----------|
| `AlertThreshold.Below` | Alert when metric < `baseline_value - delta` |
| `AlertThreshold.Above` | Alert when metric > `baseline_value + delta` |
| `AlertThreshold.Outside` | Alert when metric outside `[baseline - delta, baseline + delta]` |

### 3. Create GenAI Drift Config

Configure sampling rate, alerting schedule, and dispatch channels:

```python
from scouter import GenAIDriftConfig, GenAIAlertConfig, SlackDispatchConfig

alert_config = GenAIAlertConfig(
    dispatch_config=SlackDispatchConfig(channel="#ml-alerts"),
    schedule="0 */6 * * *",  # Every 6 hours (cron format)
    alert_condition=alert_condition
)

config = GenAIDriftConfig(
    space="production",
    name="chatbot_service",
    version="1.0.0",
    sample_ratio=1.0,  # Evaluate every request
    alert_config=alert_config
)
```

**Configuration Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `space` | `str` | `"__missing__"` | Logical grouping (e.g., "production", "staging") |
| `name` | `str` | `"__missing__"` | Service identifier |
| `version` | `str` | `"0.1.0"` | Version for this profile |
| `sample_ratio` | `float` | `1.0` | Percentage of requests to evaluate (0.0 to 1.0) |
| `alert_config` | `GenAIAlertConfig` | Default console | Alert configuration |

**Alert Dispatch Options:**

```python
# Slack
from scouter import SlackDispatchConfig
slack_config = SlackDispatchConfig(channel="#alerts")

# OpsGenie
from scouter import OpsGenieDispatchConfig
opsgenie_config = OpsGenieDispatchConfig(team="ml-ops")

# Console (logs to server console)
from scouter import ConsoleDispatchConfig
console_config = ConsoleDispatchConfig()
```

**Schedule Examples:**

```python
from scouter import CommonCrons

# Predefined schedules
alert_config = GenAIAlertConfig(schedule=CommonCrons.EveryHour)

# Custom cron expressions
alert_config = GenAIAlertConfig(schedule="0 */4 * * *")  # Every 4 hours
```

### 4. Create the Profile

Combine configuration and tasks:

```python
from scouter.evaluate import GenAIEvalProfile

profile = GenAIEvalProfile(
    config=config,
    tasks=tasks
)

# Register with Scouter server
from scouter import ScouterClient

client = ScouterClient()
client.register_profile(
    profile=profile,
    set_active=True,
    deactivate_others=False
)
```

## Inserting Records for Evaluation

Use `GenAIEvalRecord` to send evaluation data to the queue. Queue insertion is meant for real-time applications that need to monitor ML/GenAI services without blocking. In these scenarios the typical flow is:

1. Load your `ScouterQueue` with the profile path on application startup
2. For each request/response, create a `GenAIEvalRecord` with the context and insert it into the queue which will be processed asynchronously by the Scouter server.
3. The server executes the evaluation tasks on sampled records, stores results, and checks alert conditions on schedule.

### Creating Records

```python
from scouter.queue import GenAIEvalRecord

# Simple context
record = GenAIEvalRecord(
    context={
        "user_query": "How do I reset my password?",
        "response": "To reset your password, go to Settings > Security..."
    }
)

# With Pydantic models
from pydantic import BaseModel

class QueryContext(BaseModel):
    user_query: str
    response: str
    metadata: dict

context = QueryContext(
    user_query="How do I reset my password?",
    response="To reset...",
    metadata={"session_id": "abc123"}
)

record = GenAIEvalRecord(context=context)
```

### Inserting into Queue

```python
from scouter.queue import ScouterQueue

queue = ScouterQueue()

# Insert record (non-blocking)
queue["chatbot_service"].insert(record)
```

**Important:**

- Context keys must match prompt parameter names (e.g., `${user_query}` → `"user_query"`)
- Queue name must match `alias` define in ScouterQueue path configuration
- Insertion adds minimal latency to your application (nanoseconds)

## Evaluation Flow

```
┌───────────────────────────────────────────────────────────┐
│                    Your Application                       │
│                                                           │
│  ┌──────────────┐    ┌──────────────────────────────────┐ │
│  │   LLM Call   │───>│   Insert GenAIEvalRecord         │ │
│  │              │    │   to ScouterQueue (sampled)      │ │
│  └──────────────┘    └──────────┬───────────────────────┘ │
│                                 │ (async, non-blocking)   │
└─────────────────────────────────┼─────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────┐
│                   Scouter Server                            │
│                                                             │
│  1. Retrieve Profile & Tasks                                │
│  2. Execute Evaluation Tasks                                │
│     • Rebuild context for each task                         │
│     • Execute based on dependency graph                     │
│  3. Store Results                                           │
│  4. Check Alert Conditions (on schedule)                    │
│  5. Send Alerts (Slack/OpsGenie/Console)                    │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Complete Example: Multi-Category Support Agent

```python
from scouter import (
    GenAIDriftConfig,
    GenAIAlertConfig,
    AlertCondition,
    AlertThreshold,
    SlackDispatchConfig,
)
from scouter.evaluate import (
    GenAIEvalProfile,
    LLMJudgeTask,
    AssertionTask,
    ComparisonOperator,
)
from scouter.genai import Prompt, Provider, Score
from scouter.queue import ScouterQueue, GenAIEvalRecord

# 1. Define category classification
category_prompt = Prompt(
    messages="Classify this query: ${user_query}",
    model="gemini-2.5-flash-lite",
    provider=Provider.Google,
    output_type=CategoryResult
)

category_task = LLMJudgeTask(
    id="classify_category",
    prompt=category_prompt,
    expected_value=None,
    field_path="category",
    operator=ComparisonOperator.IsNotEmpty,
    condition=True
)

# 2. Technical quality path
technical_gate = AssertionTask(
    id="is_technical",
    field_path="classify_category.category",
    operator=ComparisonOperator.Equals,
    expected_value="technical",
    depends_on=["classify_category"],
    condition=True
)

technical_quality = LLMJudgeTask(
    id="technical_quality",
    prompt=technical_eval_prompt,
    expected_value=4,
    field_path="score",
    operator=ComparisonOperator.GreaterThanOrEqual,
    depends_on=["is_technical"]
)

# 3. Configure profile
alert_config = GenAIAlertConfig(
    dispatch_config=SlackDispatchConfig(channel="#support-quality"),
    schedule="0 */6 * * *",
    alert_condition=AlertCondition(
        baseline_value=4.0,
        alert_threshold=AlertThreshold.Below,
        delta=0.5
    )
)

config = GenAIDriftConfig(
    space="production",
    name="support_agent",
    version="2.0.0",
    sample_ratio=10,
    alert_config=alert_config
)

profile = GenAIEvalProfile(
    config=config,
    tasks=[category_task, technical_gate, technical_quality]
)

# 4. Register profile
from scouter import ScouterClient
client = ScouterClient()
client.register_profile(profile, set_active=True)

# 5. Use in production
queue = ScouterQueue.from_path(
    path={"support_agent": profile_path},
    ...
)

for user_query, model_response in production_requests:
    record = GenAIEvalRecord(
        context={
            "user_query": user_query,
            "response": model_response
        }
    )
    queue["support_agent"].insert(record)
```

## Best Practices

### Sampling Strategy

- **High-traffic services**: Use lower sampling rates
- **Critical metrics**: Higher sampling for important evaluations
- **Cost management**: Balance evaluation costs against monitoring needs
- **Statistical significance**: Ensure enough samples for meaningful alerts
- **Tracing integration**: Add your `ScouterQueue` to the Scouter Tracer for correlated observability, sampling and evaluation. See [Tracing Overview](/scouter/docs/tracing/overview/) for details.

### Task Design

- **Fast assertions first**: Use `AssertionTask` before expensive LLM calls
- **Conditional execution**: Use `condition=True` to gate expensive evaluations
- **Dependencies**: Chain tasks to build on previous results
- **Baseline tuning**: Set `expected_value` based on offline evaluation results

### Alert Configuration

- **Reasonable thresholds**: Base alert conditions on actual performance distributions
- **Alert frequency**: Match schedule to traffic patterns
- **Actionable alerts**: Configure thresholds that indicate real quality issues
- **Multiple channels**: Use appropriate dispatch for urgency level

## Examples

For complete working examples, see the [Scouter examples directory](https://github.com/demml/scouter/tree/main/py-scouter/examples).