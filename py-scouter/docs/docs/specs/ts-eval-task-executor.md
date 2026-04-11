# Technical Spec: Eval Task Executor

## Overview

The `TaskExecutor` is the core engine that runs evaluation tasks for a single `EvalRecord`. It manages dependency ordering, context scoping, and parallel execution across four task types: `AssertionTask`, `LLMJudgeTask`, `TraceAssertionTask`, and `AgentAssertionTask`.

---

## Execution Flow

```
AgentEvaluator::process_event_record(record, profile, spans)
        │
        ▼
Build ExecutionContext
  ├── base_context  ← EvalRecord.context (raw JSON)
  ├── assertion_store  (RwLock)
  ├── llm_response_store  (RwLock)
  └── task_registry  ← all task IDs, types, depends_on, condition flags

        │
        ▼
Build ExecutionPlan  ← topological sort of task DAG into stages
  Stage 0: tasks with no dependencies
  Stage 1: tasks whose dependencies are all in Stage 0
  Stage N: ...

        │
        ▼
For each stage (sequential):
  TaskExecutor::execute_level(stage_task_ids)
        │
        ├── DependencyChecker::filter_executable_tasks
        │     For each task: check all depends_on are complete
        │     If a conditional dependency failed → mark task skipped
        │
        └── Partition by type → run in parallel via tokio::try_join!
              ├── execute_assertions(assertion_ids)
              ├── execute_llm_judges(judge_ids)
              ├── execute_trace_assertions(trace_ids)
              └── execute_agent_assertions(agent_ids)
```

---

## Per-Task Patterns

All four task types follow the same structure:

```
1. build_scoped_context(task.depends_on)
   └── Merges base_context with upstream dependency results
       keyed by task ID (e.g. "upstream_task_id" → actual value)

2. Execute task against scoped_context

3. store_assertion(task_id, result) → assertion_store
```

The difference is what "execute" means for each type.

---

## Task Type Execution Details

### AssertionTask

```
build_scoped_context(depends_on)
        │
        ▼
AssertionEvaluator::evaluate_assertion(scoped_context, task)
        │
        ├── task.context_path set? → FieldEvaluator::extract_field_value(context, path)
        │                             └── navigates dot-notation path in scoped_context
        └── apply ComparisonOperator(actual, expected) → AssertionResult
```

`context_path` navigates to the value being compared, e.g. `"input.foo"` extracts
`scoped_context["input"]["foo"]` before applying the operator.

---

### LLMJudgeTask

```
build_scoped_context(depends_on)
        │
        ▼
workflow.execute_task(task_id, scoped_context)
        │  ← Sends scoped_context as variables to LLM prompt
        ▼
LLM response (JSON) → store in llm_response_store
        │
        ▼
AssertionEvaluator::evaluate_assertion(llm_response, judge)
        │  ← judge.context_path navigates into the LLM response JSON
        └── e.g. context_path="score" extracts response["score"]
```

The LLM response is stored separately so downstream tasks can
`depends_on: ["judge_task_id"]` and receive the full response object.

---

### TraceAssertionTask

```
TraceContextBuilder (holds span snapshot from Delta Lake)
        │
        ▼
execute_trace_assertions(builder, tasks)
        │  ← no scoped context; assertions query spans directly
        ▼
TraceAssertion variant resolves against spans
  e.g. SpanExists, TraceErrorCount, SpanSequence, ServiceMap
        │
        └── store_assertion(task_id, result)
```

Trace assertions don't use `depends_on` context injection — they query
the span store directly. The span snapshot is fixed at evaluation start.

---

### AgentAssertionTask

```
build_scoped_context(depends_on)
        │  ← same mechanism as AssertionTask / LLMJudgeTask
        ▼
AgentContextBuilder::from_context(scoped_context, provider, context_path)
        │  ← context_path here is the "response locator":
        │     where is the LLM response within scoped_context?
        │     e.g. context_path="response" → scoped_context["response"]
        │
        ├── FieldEvaluator::extract_field_value_owned(context, path)
        │     navigates to the LLM response sub-object
        │
        └── ChatResponse::from_response_value(response_val, provider)
              normalizes vendor-specific format:
              OpenAI  → choices[].message.tool_calls, usage, model
              Anthropic → content[] with ToolUseBlock, usage, model
              Gemini  → candidates[].content.parts[].function_call
        │
        ▼
AgentContextBuilder::build_context(assertion)
        │  ← resolves AgentAssertion variant to a concrete Value
        │    e.g. ToolCalled{"web_search"} → json!(true/false)
        │         ResponseContent{}       → json!("text of reply")
        │         ToolArgument{name, key} → json!(arg_value)
        │         ResponseField{path}     → path-navigate raw response
        ▼
AssertionEvaluator::evaluate_assertion(resolved_value, task)
        │  ← task.context_path() returns None here — the response
        │     locator was already consumed by from_context above.
        │     evaluate_assertion compares resolved_value directly
        │     against task.operator + task.expected_value.
        └── store_assertion(task_id, result)
```

**Key distinction**: `AgentAssertionTask.context_path` locates the LLM
response *within* the eval context (vendor response wrapper). It is separate
from `AssertionTask.context_path`, which navigates *within* the value to
find the field to compare. `TaskAccessor::context_path()` returns `None`
for `AgentAssertionTask` to prevent double-navigation.

---

## Context Scoping and `depends_on`

`build_scoped_context(depends_on)` produces a JSON object that merges:
- All top-level keys from `base_context`
- One additional key per dependency, keyed by task ID:

```
base_context: { "input": {...}, "response": {...} }
depends_on: ["check_format", "llm_judge"]

scoped_context:
{
  "input": {...},
  "response": {...},
  "check_format": <actual value from check_format AssertionResult>,
  "llm_judge": <full LLM response JSON from llm_judge>
}
```

Dependency result types:
- `AssertionTask` / `TraceAssertionTask` / `AgentAssertionTask` → injects `result.actual`
- `LLMJudgeTask` → injects the full LLM response object

If `depends_on` is empty, `base_context` is returned unchanged.

---

## Conditional Gates

A task with `condition: true` acts as a gate for downstream tasks.

```
condition_task (condition=true)
      │
      ├── passed  → downstream tasks execute normally
      └── failed  → downstream tasks are marked SKIPPED
                    (not executed, not counted in pass/fail)
```

Skipped tasks propagate: if a task is skipped, tasks that `depends_on`
it are also skipped, even if `condition` is false on the downstream task.

---

## Path Extraction

Both `AssertionTask` context navigation and `AgentAssertionTask` response
location use the same underlying engine:

```
FieldEvaluator::extract_field_value(json, path)   → &Value (borrowed)
FieldEvaluator::extract_field_value_owned(json, path) → Value (owned)
```

Supported path syntax:
- `"field"` — top-level key
- `"field.subfield"` — nested key
- `"field[0]"` — array index
- `"field[0].subfield"` — array index + nested key

Validation: paths over 512 chars or 32 segments return an error.
`AgentContextBuilder::extract_by_path` delegates to `extract_field_value_owned`.
