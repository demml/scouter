# Technical Component Specification: GenAI Evaluation

## Overview

The GenAI evaluation component provides two complementary workflows for assessing the quality, correctness, and safety of LLM-powered applications:

- **Online evaluation** — Real-time sampling and asynchronous evaluation of production inference traffic
- **Offline evaluation** — Batch evaluation of datasets for pre-deployment testing, regression analysis, and benchmarking

Both modes share a common task model (`AssertionTask`, `LLMJudgeTask`) and a common context system for extracting and passing data between evaluation steps.

---

## Online Evaluation

### How it works

Online evaluation integrates directly into the production inference path. A `GenAIEvalProfile` is registered with the Scouter server, defining what to evaluate and how. At inference time, the application inserts `EvalRecord` objects into a `ScouterQueue`. The server samples records based on a configurable `sample_ratio`, then runs evaluation tasks asynchronously via the GenAI poller background worker.

```
Inference request
      │
      ▼
Application inserts EvalRecord into ScouterQueue
      │
      ▼
Scouter server samples based on sample_ratio
      │
      ▼
GenAI poller worker picks up sampled records
      │
      ▼
Evaluation tasks run (AssertionTask / LLMJudgeTask)
      │
      ▼
Alert conditions checked → dispatch if threshold crossed
```

### EvalRecord

A `EvalRecord` captures the context of a single inference call. It accepts a Python `dict` or Pydantic `BaseModel` as context, and an optional `Prompt`.

```python
from scouter import EvalRecord

record = EvalRecord(
    context={
        "input": user_message,
        "response": model_response,
        "model": "gpt-4o",
    }
)

queue["my-genai-profile"].insert(record)
```

### GenAIEvalProfile

The profile defines the evaluation configuration, including tasks, alert thresholds, and sampling ratio.

```python
from scouter.evaluate import GenAIEvalProfile, GenAIEvalConfig
from scouter.alert import ConsoleDispatchConfig
from scouter.types import CommonCrons

profile = GenAIEvalProfile(
    config=GenAIEvalConfig(
        space="my-space",
        name="my-model",
        version="1.0.0",
        sample_ratio=0.1,           # Evaluate 10% of production traffic
        alert_config=ConsoleDispatchConfig(
            schedule=CommonCrons.EveryHour,
        ),
    ),
    tasks=[assertion_task, judge_task],
)
```

---

## Offline Evaluation

Offline evaluation runs batch evaluation against a `EvalDataset`. This is useful for pre-deployment testing, regression analysis, and comparing model versions.

### Workflow

```python
from scouter.evaluate import EvalDataset, GenAIEvalConfig

dataset = EvalDataset(
    config=GenAIEvalConfig(
        space="my-space",
        name="my-model",
        version="1.0.0",
    ),
    tasks=[assertion_task, judge_task],
    records=[record_1, record_2, ...],
)

results = dataset.evaluate()
```

---

## Evaluation Tasks

### AssertionTask

Deterministic, rule-based evaluation. Supports 50+ `ComparisonOperator` values covering string, numeric, collection, and JSON checks.

```python
from scouter.evaluate import AssertionTask, ComparisonOperator

task = AssertionTask(
    task_id="check_json_response",
    context_path="response",                  # dot-notation field extraction
    operator=ComparisonOperator.IsJson,
    condition=True,                           # acts as a gate: skip downstream tasks on failure
)
```

**Key features:**
- `context_path`: Dot-notation path to extract a field from the record context (e.g. `"response.choices.0.message.content"`)
- `condition=True`: When set, a failed assertion skips all downstream tasks that depend on this one
- Template variable substitution: use `${field_name}` in expected values to reference context fields

### LLMJudgeTask

LLM-powered semantic evaluation. Sends context and a prompt to an LLM and extracts a structured score using Pydantic models.

```python
from scouter.evaluate import LLMJudgeTask
from scouter.genai import Prompt

task = LLMJudgeTask(
    task_id="judge_helpfulness",
    prompt=Prompt(
        message="Rate the helpfulness of the following response on a scale of 1–5.\n\nUser: ${input}\nResponse: ${response}",
    ),
    context_path="score",                    # extract 'score' field from LLM response
    depends_on=["check_json_response"],      # only run if assertion gate passed
)
```

**Supported LLM providers:**
- OpenAI (via `OPENAI_API_KEY`)
- Anthropic (via `ANTHROPIC_API_KEY`)
- Google Gemini (via `GOOGLE_API_KEY` / service account)

---

## Task Dependencies

Tasks can declare `depends_on` to form a dependency graph. Each task receives the base record context plus the outputs of all declared dependencies.

```python
tasks = [
    AssertionTask(task_id="validate_format", context_path="response", operator=ComparisonOperator.IsJson, condition=True),
    LLMJudgeTask(task_id="judge_quality", depends_on=["validate_format"], ...),
    AssertionTask(task_id="check_score", context_path="judge_quality.score", operator=ComparisonOperator.GreaterThan, expected=3, depends_on=["judge_quality"]),
]
```

---

## Background Worker: GenAI Poller

The `GenAIPollerSettings` controls the server-side worker that processes queued evaluation records.

| Setting | Env Var | Default | Description |
|---------|---------|---------|-------------|
| Worker count | `GENAI_WORKER_COUNT` | `2` | Number of concurrent evaluation workers |
| Max retries | `GENAI_MAX_RETRIES` | `3` | Retries on task failure |
| Trace wait timeout | `GENAI_TRACE_WAIT_TIMEOUT_SECS` | `10` | Seconds to wait for an associated trace before giving up |
| Trace backoff | `GENAI_TRACE_BACKOFF_MILLIS` | `100` | Delay between trace polling attempts |
| Reschedule delay | `GENAI_TRACE_RESCHEDULE_DELAY_SECS` | `30` | Delay before rescheduling a failed evaluation task |

---

*Version: 1.0*
*Last Updated: 2026-03-01*
*Component Owner: Steven Forrester*
