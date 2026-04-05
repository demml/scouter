# Online evaluation

Online evaluation runs the same evaluation tasks as offline, but against sampled production traffic. Evaluations run asynchronously on the Scouter server with no impact on application latency.

Use it to catch quality degradation and distribution shift after deployment. The task definitions are identical to offline; write them once and use them in both contexts.

## What is a GenAI drift profile?

A `GenAIEvalProfile` for online use pairs a `GenAIEvalConfig` (service metadata and alert settings) with your evaluation tasks (`LLMJudgeTask`, `AssertionTask`). The profile runs your tasks asynchronously on sampled traffic, stores results, and checks alert conditions on a configured schedule.

## Creating a GenAI drift profile

### 1. Define evaluation tasks

Use the same `LLMJudgeTask` and `AssertionTask` patterns from offline evaluation. Tasks support dependencies and conditional logic.

#### Simple LLM judge

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
    context_path="score",
    operator=ComparisonOperator.GreaterThanOrEqual,
    description="Ensure relevance score >= 4"
)
```

#### Hybrid evaluation

```python
from scouter.evaluate import AssertionTask

# Fast assertion check
length_check = AssertionTask(
    id="response_not_empty",
    context_path="response",
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
    context_path="score",
    operator=ComparisonOperator.GreaterThanOrEqual,
    depends_on=["response_not_empty"],
    description="Quality must be >= 4"
)

tasks = [length_check, quality_task]
```

#### Multi-stage dependent evaluation

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
    context_path="category",
    operator=ComparisonOperator.IsNotEmpty,
    description="Classify query category"
)

# Stage 2: Technical accuracy (only for technical queries)
technical_check = AssertionTask(
    id="is_technical",
    context_path="category_classification.category",
    operator=ComparisonOperator.Equals,
    expected_value="technical",
    depends_on=["category_classification"],
    condition=True
)

technical_quality_task = LLMJudgeTask(
    id="technical_quality",
    prompt=technical_quality_prompt,
    expected_value=4,
    context_path="score",
    operator=ComparisonOperator.GreaterThanOrEqual,
    depends_on=["is_technical"],
    description="Technical accuracy >= 4"
)

tasks = [category_task, technical_check, technical_quality_task]
```

### 2. Configure alert conditions

Define when alerts should trigger using `AlertCondition`. Alerts fire based on the overall pass rate across your evaluation tasks. With multiple tasks, the alert condition evaluates the aggregated workflow results.

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

### 3. Create GenAI drift config

Configure sampling rate, alerting schedule, and dispatch channels:

```python
from scouter import GenAIEvalConfig, GenAIAlertConfig, SlackDispatchConfig

alert_config = GenAIAlertConfig(
    dispatch_config=SlackDispatchConfig(channel="#ml-alerts"),
    schedule="0 */6 * * *",  # Every 6 hours (cron format)
    alert_condition=alert_condition
)

config = GenAIEvalConfig(
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

### 4. Create the profile

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

## Inserting records for evaluation

For each request your service handles, create an `EvalRecord` and insert it into the queue. The queue is non-blocking; insertion takes nanoseconds and doesn't affect your application's response time. The Scouter server picks up records asynchronously and evaluates the sampled ones.

The typical setup:
1. Load your `ScouterQueue` on application startup
2. For each request/response, insert an `EvalRecord` into the queue
3. The server evaluates sampled records, stores results, and checks alert conditions on schedule

### Creating records

```python
from scouter.queue import EvalRecord

# Simple context
record = EvalRecord(
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

record = EvalRecord(context=context)
```

### Inserting into queue

```python
from scouter.queue import ScouterQueue

queue = ScouterQueue()

# Insert record (non-blocking)
queue["chatbot_service"].insert(record)
```

Context keys must match the `${variable}` names in your prompt templates (e.g., `${user_query}` requires `"user_query"` in the context dict). The queue name must match the `alias` in your `ScouterQueue` path configuration.

## Evaluation flow

```
┌───────────────────────────────────────────────────────────┐
│                    Your Application                       │
│                                                           │
│  ┌──────────────┐    ┌──────────────────────────────────┐ │
│  │   LLM Call   │───>│   Insert EvalRecord         │ │
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

## Complete example: multi-category support agent

```python
from scouter import (
    GenAIEvalConfig,
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
from scouter.queue import ScouterQueue, EvalRecord

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
    context_path="category",
    operator=ComparisonOperator.IsNotEmpty,
    condition=True
)

# 2. Technical quality path
technical_gate = AssertionTask(
    id="is_technical",
    context_path="classify_category.category",
    operator=ComparisonOperator.Equals,
    expected_value="technical",
    depends_on=["classify_category"],
    condition=True
)

technical_quality = LLMJudgeTask(
    id="technical_quality",
    prompt=technical_eval_prompt,
    expected_value=4,
    context_path="score",
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

config = GenAIEvalConfig(
    space="production",
    name="support_agent",
    version="2.0.0",
    sample_ratio=0.1,  # evaluate 10% of requests
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
    record = EvalRecord(
        context={
            "user_query": user_query,
            "response": model_response
        }
    )
    queue["support_agent"].insert(record)
```

## Best practices

**Sampling**: High-traffic services should use lower `sample_ratio` values. For statistically meaningful alerts, ensure you're collecting enough samples per evaluation window. The right number depends on your traffic volume and how tight your thresholds are. To correlate evaluations with distributed traces, pass your `ScouterQueue` to the Scouter tracer. See [Tracing overview](/scouter/docs/tracing/overview/).

**Task design**: Lead with `AssertionTask` before `LLMJudgeTask`. Use `condition=True` to skip expensive LLM calls when cheap preconditions fail. Set `expected_value` thresholds based on what you observed in offline evaluation runs, not guesses.

**Alert thresholds**: Base `baseline_value` and `delta` on real performance distributions. Thresholds set without data tend to alert on noise or miss real regressions. Match the alert schedule to your traffic pattern; hourly evaluation on a low-traffic service produces unreliable signal.

## Examples

For complete working examples, see the [Scouter examples directory](https://github.com/demml/scouter/tree/main/py-scouter/examples).