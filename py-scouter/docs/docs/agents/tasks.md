# Evaluation Tasks: Building Blocks for GenAI Evaluation

Evaluation tasks are the core building blocks of Scouter's GenAI evaluation framework. They define what to evaluate, how to evaluate it, and what conditions must be met for an evaluation to pass. Tasks can be combined with dependencies to create sophisticated evaluation workflows that handle complex, multi-stage assessments.

## Task Types

Scouter provides four types of evaluation tasks:

| Task Type | Evaluation Method | Use Cases | Cost/Latency |
|-----------|------------------|-----------|--------------|
| **AssertionTask** | Deterministic rule-based validation | Structure validation, threshold checks, pattern matching | Zero cost, minimal latency |
| **LLMJudgeTask** | LLM-powered reasoning | Semantic similarity, quality assessment, complex criteria | Additional LLM call cost and latency |
| **TraceAssertionTask** | Trace/span property validation | Execution order, retry counts, token budgets, SLA enforcement | Zero cost, minimal latency |
| **AgentAssertionTask** | Deterministic agent response validation | Tool call verification, argument matching, token counts, response content | Zero cost, minimal latency |

All task types support:
- **Dependencies**: Chain tasks to build on previous results
- **Conditional execution**: Use tasks as gates to control downstream evaluation
- **context path extraction**: Access nested values in context or upstream outputs

## AssertionTask

`AssertionTask` performs fast, deterministic validation without requiring additional LLM calls. Assertions evaluate values from the evaluation context against expected conditions using comparison operators.

### Parameters

```python
AssertionTask(
    id: str,
    expected_value: Any,
    operator: ComparisonOperator,
    context_path: Optional[str] = None,
    description: Optional[str] = None,
    depends_on: Optional[List[str]] = None,
    condition: bool = False,
)
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id` | `str` | Yes | Unique identifier (converted to lowercase). Used in dependencies and results. |
| `expected_value` | `Any` | Yes | Value to compare against. Supports static values (str, int, float, bool, list, dict, None) or template references to context fields using `${field.path}` syntax. |
| `operator` | `ComparisonOperator` | Yes | Comparison operator to use for evaluation. |
| `context_path` | `str` | No | Dot-notation path to extract value from context (e.g., `"response.confidence"`). If `None`, uses entire context. |
| `description` | `str` | No | Human-readable description for understanding results. |
| `depends_on` | `List[str]` | No | Task IDs that must complete before this task executes. Outputs from dependencies are added to context. |
| `condition` | `bool` | No | If `True`, acts as conditional gate. Failed conditions skip dependent tasks and exclude this task from final results. |

### Template Variable Substitution

The `expected_value` parameter supports **template variable substitution** to reference values from the evaluation context dynamically. This is particularly useful for comparing against ground truth values or data provided at runtime.

**Template Syntax:**
- Use `${field.path}` to reference a field in the context
- Supports nested paths: `${ground_truth.category}`, `${metadata.expected_score}`
- Works with all comparison operators and data types
- Resolved at evaluation time with proper error handling

**Use Cases:**
- **Ground Truth Comparison**: Compare model predictions against reference values
- **Dynamic Thresholds**: Use runtime-provided thresholds from metadata
- **Cross-Field Validation**: Compare one field against another in the same context

### Common Patterns

**Static Threshold Validation:**

```python
AssertionTask(
    id="confidence_check",
    context_path="model_output.confidence",
    operator=ComparisonOperator.GreaterThanOrEqual,
    expected_value=0.85,
    description="Require confidence >= 85%"
)
```

**Ground Truth Comparison (Template):**

```python
# Context: {"prediction": "electronics", "ground_truth": "electronics"}
AssertionTask(
    id="correct_category",
    context_path="prediction",
    operator=ComparisonOperator.Equals,
    expected_value="${ground_truth}",  # Template reference
    description="Verify prediction matches ground truth"
)
```

**Nested Ground Truth Reference:**

```python
# Context: {
#   "agent_classification": {"category": "kitchen"},
#   "ground_truth": {"category": "kitchen", "confidence": 0.9}
# }
AssertionTask(
    id="validate_category",
    context_path="agent_classification.category",
    operator=ComparisonOperator.Equals,
    expected_value="${ground_truth.category}",  # Nested template
    description="Compare against nested ground truth"
)
```

**Dynamic Threshold from Metadata:**

```python
# Context: {
#   "score": 8.5,
#   "thresholds": {"min_score": 7.0}
# }
AssertionTask(
    id="score_threshold",
    context_path="score",
    operator=ComparisonOperator.GreaterThanOrEqual,
    expected_value="${thresholds.min_score}",  # Dynamic threshold
    description="Score must exceed dynamic threshold"
)
```

**Array/List Comparison with Template:**

```python
# Context: {
#   "recommended_products": ["Bosch", "Miele"],
#   "ground_truth": {"expected_brands": ["Bosch", "Miele", "LG"]}
# }
AssertionTask(
    id="validate_products",
    context_path="recommended_products",
    operator=ComparisonOperator.ContainsAny,
    expected_value="${ground_truth.expected_brands}",
    description="Recommendations must include expected brands"
)
```

**Structure Validation (Static):**

```python
AssertionTask(
    id="has_required_fields",
    context_path="response",
    operator=ComparisonOperator.ContainsAll,
    expected_value=["answer", "sources", "confidence"],
    description="Ensure response has all required fields"
)
```

**Conditional Gate (Static):**

```python
AssertionTask(
    id="is_production_ready",
    context_path="metadata.environment",
    operator=ComparisonOperator.Equals,
    expected_value="production",
    condition=True,  # Downstream tasks only run if this passes
    description="Gate for production-only evaluations"
)
```

**Dependent Assertion with Template:**

```python
# Depends on upstream LLMJudgeTask that adds output to context
AssertionTask(
    id="technical_score_high",
    context_path="expert_validation.technical_score",
    operator=ComparisonOperator.GreaterThan,
    expected_value="${quality_thresholds.technical}",  # Template from context
    depends_on=["expert_validation"],
    description="Technical score must exceed configured threshold"
)
```

### Template Error Handling

If a template variable cannot be resolved it will raise an error

```python
# Context: {"prediction": "electronics"}
# Missing "ground_truth" field

AssertionTask(
    id="correct_category",
    context_path="prediction",
    operator=ComparisonOperator.Equals,
    expected_value="${ground_truth}",  # Will fail
)
```

**Best Practices:**
- Use templates for dynamic comparisons against runtime data
- Use static values for fixed thresholds and validation rules
- Validate that template fields exist in your context structure
- Use descriptive field paths in templates for clarity
- Combine with `depends_on` to ensure upstream data is available

## LLMJudgeTask

`LLMJudgeTask` uses an additional LLM call to evaluate responses based on sophisticated criteria requiring reasoning, context understanding, or subjective judgment. LLM judges are ideal for evaluations that cannot be captured by deterministic rules.

### Parameters

```python
LLMJudgeTask(
    id: str,
    prompt: Prompt,
    expected_value: Any,
    context_path: Optional[str],
    operator: ComparisonOperator,
    description: Optional[str] = None,
    depends_on: Optional[List[str]] = None,
    max_retries: Optional[int] = None,
    condition: bool = False,
)
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id` | `str` | Yes | Unique identifier (converted to lowercase). |
| `prompt` | `Prompt` | Yes | Prompt configuration defining the LLM evaluation. Must use parameter binding (e.g., `${user_query}`). |
| `expected_value` | `Any` | Yes | Value to compare against LLM response. Type depends on prompt's `output_type`. |
| `context_path` | `str` | No | Dot-notation path to extract from LLM response (e.g., `"score"`). If `None`, uses entire response. |
| `operator` | `ComparisonOperator` | Yes | Comparison operator for evaluating LLM response against expected value. |
| `description` | `str` | No | Human-readable description of evaluation purpose. |
| `depends_on` | `List[str]` | No | Task IDs that must complete first. Dependency outputs are added to context and available in prompt. |
| `max_retries` | `int` | No | Maximum retry attempts for LLM call failures (network errors, rate limits). Defaults to 3. |
| `condition` | `bool` | No | If `True`, acts as conditional gate for dependent tasks. |

### Common Patterns

**Basic Quality Assessment:**

```python
from scouter.agent import Score

quality_prompt = Prompt(
    messages=(
        "Rate the quality of this response on a scale of 1-5.\n\n"
        "Query: ${user_query}\n"
        "Response: ${response}\n\n"
        "Consider clarity, completeness, and accuracy."
    ),
    model="gpt-4o-mini",
    provider=Provider.OpenAI,
    output_type=Score  # Returns {"score": float, "reason": str}
)

LLMJudgeTask(
    id="quality_assessment",
    prompt=quality_prompt,
    expected_value=4,
    context_path="score",
    operator=ComparisonOperator.GreaterThanOrEqual,
    description="Quality must be >= 4"
)
```

**Semantic Similarity Check:**

```python
similarity_prompt = Prompt(
    messages=(
        "Compare the semantic similarity between the generated answer and reference answer.\n\n"
        "Generated: ${generated_answer}\n"
        "Reference: ${reference_answer}\n\n"
        "Rate similarity from 0 (completely different) to 10 (identical meaning)."
    ),
    model="claude-3-5-sonnet-20241022",
    provider=Provider.Anthropic,
    output_type=Score
)

LLMJudgeTask(
    id="semantic_similarity",
    prompt=similarity_prompt,
    expected_value=7,
    context_path="score",
    operator=ComparisonOperator.GreaterThanOrEqual,
    description="Semantic similarity >= 7"
)
```

**Conditional Judge with Dependencies:**

```python
# Only evaluate toxicity if response passes length check
toxicity_prompt = Prompt(
    messages="Evaluate toxicity level (0-10): ${response}",
    model="gemini-2.5-flash-lite",
    provider=Provider.Google,
    output_type=Score
)

LLMJudgeTask(
    id="toxicity_check",
    prompt=toxicity_prompt,
    expected_value=2,
    context_path="score",
    operator=ComparisonOperator.LessThanOrEqual,
    depends_on=["length_check"],  # Only runs if length_check passes
    description="Toxicity must be <= 2"
)
```

**Multi-Stage Evaluation:**

```python
# Stage 1: Relevance
relevance_task = LLMJudgeTask(
    id="relevance",
    prompt=relevance_prompt,
    expected_value=7,
    context_path="score",
    operator=ComparisonOperator.GreaterThanOrEqual
)

# Stage 2: Factuality (depends on relevance)
factuality_task = LLMJudgeTask(
    id="factuality",
    prompt=factuality_prompt,
    expected_value=8,
    context_path="score",
    operator=ComparisonOperator.GreaterThanOrEqual,
    depends_on=["relevance"],
    description="Factuality check after relevance validation"
)
```

## TraceAssertionTask

`TraceAssertionTask` validates properties of distributed traces — span execution order, span counts, attribute values, durations, and numeric aggregations. Unlike `AssertionTask`, which evaluates values from a flat evaluation context, `TraceAssertionTask` operates directly on trace data captured by Scouter's tracing system.

Use it when you need to verify *how* an agent or pipeline executed, not just *what* it returned.

### Parameters

```python
TraceAssertionTask(
    id: str,
    assertion: TraceAssertion,
    expected_value: Any,
    operator: ComparisonOperator,
    description: Optional[str] = None,
    depends_on: Optional[List[str]] = None,
    condition: bool = False,
)
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id` | `str` | Yes | Unique identifier (converted to lowercase). |
| `assertion` | `TraceAssertion` | Yes | What to extract or measure from the trace. |
| `expected_value` | `Any` | Yes | Value to compare against the assertion result. |
| `operator` | `ComparisonOperator` | Yes | How to compare the extracted value against `expected_value`. |
| `description` | `str` | No | Human-readable description for understanding results. |
| `depends_on` | `List[str]` | No | Task IDs that must complete before this task executes. |
| `condition` | `bool` | No | If `True`, acts as a conditional gate. Failed conditions skip dependent tasks. |

### TraceAssertion

`TraceAssertion` defines what to extract from a trace. Assertions fall into two categories: **span-level** (operate on a filtered subset of spans) and **trace-level** (aggregate over the full trace).

**Span-level assertions** — require a `SpanFilter`:

| Assertion | Returns | Use case |
|-----------|---------|----------|
| `TraceAssertion.span_exists(filter)` | `bool` | Check a matching span exists |
| `TraceAssertion.span_count(filter)` | `int` | Count matching spans |
| `TraceAssertion.span_duration(filter)` | `float` (ms) | Duration of the first matching span |
| `TraceAssertion.span_attribute(filter, attribute_key)` | `Any` | Attribute value from the first matching span |
| `TraceAssertion.span_aggregation(filter, attribute_key, aggregation)` | `float` | Aggregate a numeric attribute across matching spans |
| `TraceAssertion.span_sequence(names)` | `bool` | Verify spans executed in the given order |
| `TraceAssertion.span_set(names)` | `bool` | Verify all named spans exist (any order) |

**Trace-level assertions** — no filter required:

| Assertion | Returns | Use case |
|-----------|---------|----------|
| `TraceAssertion.trace_duration()` | `float` (ms) | Total trace wall-clock time |
| `TraceAssertion.trace_span_count()` | `int` | Total number of spans in the trace |
| `TraceAssertion.trace_error_count()` | `int` | Number of spans with ERROR status |
| `TraceAssertion.trace_service_count()` | `int` | Number of unique services in the trace |
| `TraceAssertion.trace_max_depth()` | `int` | Maximum span nesting depth |
| `TraceAssertion.trace_attribute(attribute_key)` | `Any` | Attribute value from the root span |

### SpanFilter

`SpanFilter` selects which spans a span-level assertion targets. Filters compose with `.and_()` and `.or_()`.

| Filter | Description |
|--------|-------------|
| `SpanFilter.by_name(name)` | Exact span name match |
| `SpanFilter.by_name_pattern(pattern)` | Regex match against span name |
| `SpanFilter.with_attribute(key)` | Has attribute with the given key |
| `SpanFilter.with_attribute_value(key, value)` | Attribute key equals value |
| `SpanFilter.with_status(status)` | Match span status (`SpanStatus.Ok`, `Error`, or `Unset`) |
| `SpanFilter.with_duration(min_ms, max_ms)` | Duration within range (either bound is optional) |
| `SpanFilter.sequence(names)` | Matches spans that form part of a named sequence |

**Combining filters:**

```python
# Spans named "llm.generate" that completed successfully
SpanFilter.by_name("llm.generate").and_(SpanFilter.with_status(SpanStatus.Ok))

# Spans with a POST method or the "gpt-4" model attribute
SpanFilter.with_attribute_value("http.method", "POST").or_(
    SpanFilter.with_attribute_value("model", "gpt-4")
)

# (pattern match AND has attribute) OR error status
SpanFilter.by_name_pattern(r"llm\..*").and_(
    SpanFilter.with_attribute("model")
).or_(SpanFilter.with_status(SpanStatus.Error))
```

### AggregationType

Used with `TraceAssertion.span_aggregation()` to compute a value over a numeric attribute across multiple matching spans.

| Value | Description |
|-------|-------------|
| `AggregationType.Count` | Number of matching spans |
| `AggregationType.Sum` | Sum of attribute values across spans |
| `AggregationType.Average` | Mean of attribute values |
| `AggregationType.Min` | Minimum attribute value |
| `AggregationType.Max` | Maximum attribute value |
| `AggregationType.First` | Value from the first matching span |
| `AggregationType.Last` | Value from the last matching span |

### Common Patterns

**Verify agent execution order:**

```python
from scouter.evaluate import TraceAssertionTask, TraceAssertion, ComparisonOperator

TraceAssertionTask(
    id="verify_workflow",
    assertion=TraceAssertion.span_sequence(["call_tool", "run_agent", "double_check"]),
    operator=ComparisonOperator.Equals,
    expected_value=True,
    description="Verify correct agent execution order",
)
```

**Enforce retry limits:**

```python
TraceAssertionTask(
    id="limit_retries",
    assertion=TraceAssertion.span_count(SpanFilter.by_name("retry_operation")),
    operator=ComparisonOperator.LessThanOrEqual,
    expected_value=3,
    description="No more than 3 retry attempts",
)
```

**Verify correct model was used:**

```python
TraceAssertionTask(
    id="verify_model",
    assertion=TraceAssertion.span_attribute(
        filter=SpanFilter.by_name("llm.generate"),
        attribute_key="model",
    ),
    operator=ComparisonOperator.Equals,
    expected_value="gpt-4",
    description="Verify gpt-4 was used for generation",
)
```

**Enforce token budget across all LLM calls:**

```python
TraceAssertionTask(
    id="token_budget",
    assertion=TraceAssertion.span_aggregation(
        filter=SpanFilter.by_name_pattern(r"llm\..*"),
        attribute_key="token_count",
        aggregation=AggregationType.Sum,
    ),
    operator=ComparisonOperator.LessThan,
    expected_value=10000,
    description="Total tokens must stay under budget",
)
```

**Enforce performance SLA:**

```python
TraceAssertionTask(
    id="performance_sla",
    assertion=TraceAssertion.trace_duration(),
    operator=ComparisonOperator.LessThan,
    expected_value=5000.0,  # 5 seconds in ms
    description="Execution must complete within 5 seconds",
)
```

**Error-free execution:**

```python
TraceAssertionTask(
    id="no_errors",
    assertion=TraceAssertion.trace_error_count(),
    operator=ComparisonOperator.Equals,
    expected_value=0,
    description="Verify error-free execution",
)
```

**Chained assertions with conditional gate:**

```python
# Step 1: verify execution order — gate for downstream tasks
workflow_task = TraceAssertionTask(
    id="workflow_check",
    assertion=TraceAssertion.span_sequence(["validate", "process", "output"]),
    operator=ComparisonOperator.Equals,
    expected_value=True,
    condition=True,  # Skip downstream tasks if order is wrong
)

# Step 2: check performance only if workflow passed
perf_task = TraceAssertionTask(
    id="performance_check",
    assertion=TraceAssertion.trace_duration(),
    operator=ComparisonOperator.LessThan,
    expected_value=3000.0,
    depends_on=["workflow_check"],
)

# Step 3: check for errors only after both above pass
error_task = TraceAssertionTask(
    id="error_check",
    assertion=TraceAssertion.trace_error_count(),
    operator=ComparisonOperator.Equals,
    expected_value=0,
    depends_on=["workflow_check", "performance_check"],
)
```

### Offline Evaluation

For batch offline evaluation against captured spans, use `execute_trace_assertion_tasks`:

```python
from scouter.evaluate import (
    execute_trace_assertion_tasks,
    TraceAssertionTask,
    TraceAssertion,
    SpanFilter,
    ComparisonOperator,
)

results = execute_trace_assertion_tasks(
    tasks=[
        TraceAssertionTask(
            id="check_span_exists",
            assertion=TraceAssertion.span_exists(
                filter=SpanFilter.by_name("child_1"),
            ),
            operator=ComparisonOperator.Equals,
            expected_value=True,
        ),
        TraceAssertionTask(
            id="check_pattern_count",
            assertion=TraceAssertion.span_count(
                filter=SpanFilter.by_name_pattern(r"^child_.*"),
            ),
            operator=ComparisonOperator.Equals,
            expected_value=2,
        ),
    ],
    spans=captured_spans,  # list of spans from a trace
)

for task_id, result in results.items():
    print(f"{task_id}: {'PASS' if result.passed else 'FAIL'} (actual={result.actual})")
```

## AgentAssertionTask

`AgentAssertionTask` evaluates LLM agent responses by asserting on tool call behavior and response properties. It is deterministic — no additional LLM call is required. Each task defines an `AgentAssertion` (what to extract from the agent context), a comparison operator, and an expected value.

Vendor format is auto-detected from the response structure:

| Vendor | Detection signal |
|--------|-----------------|
| OpenAI | `choices` key present |
| Anthropic | `stop_reason` + `content` keys present |
| Google / Vertex | `candidates` key present |

The context passed to `execute_agent_assertion_tasks()` can be the raw response object directly, or a dict with a `"response"` sub-key — both are handled automatically.

### Import

```python
from scouter.evaluate import AgentAssertion, AgentAssertionTask, execute_agent_assertion_tasks
```

### Parameters

```python
AgentAssertionTask(
    id: str,
    assertion: AgentAssertion,
    expected_value: Any,
    operator: ComparisonOperator,
    description: Optional[str] = None,
    depends_on: Optional[List[str]] = None,
    condition: Optional[bool] = None,
)
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id` | `str` | Yes | Unique identifier (converted to lowercase). |
| `assertion` | `AgentAssertion` | Yes | Defines what to extract from the agent interaction. |
| `expected_value` | `Any` | Yes | Value to compare against the extracted result. Must be JSON-serializable. |
| `operator` | `ComparisonOperator` | Yes | How to compare the extracted value against `expected_value`. |
| `description` | `str` | No | Human-readable description of what this task checks. |
| `depends_on` | `List[str]` | No | Task IDs whose outputs this task may reference. |
| `condition` | `bool` | No | If `True`, this task acts as a gate — downstream tasks are skipped if this task fails. |

### AgentAssertion Variants

#### Tool Call Assertions

| Constructor | Returns | Description |
|-------------|---------|-------------|
| `AgentAssertion.tool_called(name)` | `bool` | Whether the named tool was called at all. |
| `AgentAssertion.tool_not_called(name)` | `bool` | Whether the named tool was NOT called. |
| `AgentAssertion.tool_called_with_args(name, arguments)` | `bool` | Whether the tool was called with arguments that are a superset of `arguments` (partial match). |
| `AgentAssertion.tool_call_sequence(names)` | `bool` | Whether tools were called in the exact order given by `names`. |
| `AgentAssertion.tool_call_count(name=None)` | `int` | Number of times the named tool was called, or total tool call count if `name` is `None`. |
| `AgentAssertion.tool_argument(name, argument_key)` | `Any` | Value of a specific argument from a tool call. |
| `AgentAssertion.tool_result(name)` | `Any` | Return value from a tool call. |

#### Response Assertions

| Constructor | Returns | Description |
|-------------|---------|-------------|
| `AgentAssertion.response_content()` | `str` | Text content of the response. |
| `AgentAssertion.response_model()` | `str` | Model identifier used in the response. |
| `AgentAssertion.response_finish_reason()` | `str` | Finish/stop reason of the response. |
| `AgentAssertion.response_input_tokens()` | `int` | Input token count. |
| `AgentAssertion.response_output_tokens()` | `int` | Output token count. |
| `AgentAssertion.response_total_tokens()` | `int` | Total token count. |
| `AgentAssertion.response_field(path)` | `Any` | Arbitrary field extracted via dot-notation from the response value. Useful for vendor-specific fields not covered by the named variants. |

`response_field(path)` resolves its path against the response value, not the full context root. For example, `response_field("choices[0].message.content")` is equivalent to `response["choices"][0]["message"]["content"]`.

### Standalone Usage

Use `execute_agent_assertion_tasks()` to evaluate tasks against a response outside of any dataset or profile.

**OpenAI format:**

```python
from scouter.evaluate import (
    AgentAssertion,
    AgentAssertionTask,
    ComparisonOperator,
    execute_agent_assertion_tasks,
)

# Raw OpenAI response
response = {
    "model": "gpt-4o",
    "choices": [
        {
            "message": {
                "role": "assistant",
                "content": None,
                "tool_calls": [
                    {
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "web_search",
                            "arguments": '{"query": "weather NYC", "lang": "en"}',
                        },
                    },
                    {
                        "id": "call_2",
                        "type": "function",
                        "function": {"name": "summarize", "arguments": "{}"},
                    },
                ],
            },
            "finish_reason": "tool_calls",
        }
    ],
    "usage": {"prompt_tokens": 120, "completion_tokens": 80, "total_tokens": 200},
}

results = execute_agent_assertion_tasks(
    tasks=[
        AgentAssertionTask(
            id="search_called",
            assertion=AgentAssertion.tool_called("web_search"),
            expected_value=True,
            operator=ComparisonOperator.Equals,
            description="Verify web_search was invoked",
        ),
        AgentAssertionTask(
            id="no_delete",
            assertion=AgentAssertion.tool_not_called("delete_user"),
            expected_value=True,
            operator=ComparisonOperator.Equals,
        ),
        AgentAssertionTask(
            id="search_args",
            assertion=AgentAssertion.tool_called_with_args("web_search", {"query": "weather NYC"}),
            expected_value=True,
            operator=ComparisonOperator.Equals,
            description="Partial argument match",
        ),
        AgentAssertionTask(
            id="call_order",
            assertion=AgentAssertion.tool_call_sequence(["web_search", "summarize"]),
            expected_value=True,
            operator=ComparisonOperator.Equals,
        ),
        AgentAssertionTask(
            id="token_budget",
            assertion=AgentAssertion.response_total_tokens(),
            expected_value=500,
            operator=ComparisonOperator.LessThan,
        ),
    ],
    context={"response": response},
)

for task_id, result in results.results.items():
    print(f"{task_id}: {'PASS' if result.passed else 'FAIL'} (actual={result.actual})")
```

**Anthropic format:**

```python
response = {
    "id": "msg_01",
    "type": "message",
    "role": "assistant",
    "model": "claude-sonnet-4-20250514",
    "content": [
        {"type": "text", "text": "Let me search for that."},
        {
            "type": "tool_use",
            "id": "toolu_01",
            "name": "web_search",
            "input": {"query": "weather NYC"},
        },
    ],
    "stop_reason": "tool_use",
    "usage": {"input_tokens": 100, "output_tokens": 200},
}

results = execute_agent_assertion_tasks(
    tasks=[
        AgentAssertionTask(
            id="model_check",
            assertion=AgentAssertion.response_model(),
            expected_value="claude-sonnet-4-20250514",
            operator=ComparisonOperator.Equals,
        ),
        AgentAssertionTask(
            id="search_called",
            assertion=AgentAssertion.tool_called("web_search"),
            expected_value=True,
            operator=ComparisonOperator.Equals,
        ),
        AgentAssertionTask(
            id="output_tokens",
            assertion=AgentAssertion.response_output_tokens(),
            expected_value=500,
            operator=ComparisonOperator.LessThan,
        ),
    ],
    context=response,  # can also pass the response directly (no "response" wrapper)
)
```

### EvalDataset Integration

`AgentAssertionTask` can be combined with other task types in an `EvalDataset` for offline batch evaluation:

```python
from scouter.evaluate import (
    AgentAssertion,
    AgentAssertionTask,
    AssertionTask,
    ComparisonOperator,
    EvalDataset,
    EvalRecord,
)

records = [
    EvalRecord(
        context={
            "response": {
                "model": "gpt-4o",
                "choices": [
                    {
                        "message": {
                            "content": "The capital of France is Paris.",
                            "tool_calls": [],
                        },
                        "finish_reason": "stop",
                    }
                ],
                "usage": {"prompt_tokens": 30, "completion_tokens": 15, "total_tokens": 45},
            },
            "expected_answer": "Paris",
        }
    ),
]

tasks = [
    # Gate: only proceed if the model finished normally
    AgentAssertionTask(
        id="finish_reason_gate",
        assertion=AgentAssertion.response_finish_reason(),
        expected_value="stop",
        operator=ComparisonOperator.Equals,
        condition=True,
    ),
    # Verify response content contains the expected answer
    AgentAssertionTask(
        id="answer_check",
        assertion=AgentAssertion.response_content(),
        expected_value="${expected_answer}",
        operator=ComparisonOperator.Contains,
        depends_on=["finish_reason_gate"],
    ),
    # Enforce token budget
    AgentAssertionTask(
        id="token_budget",
        assertion=AgentAssertion.response_total_tokens(),
        expected_value=200,
        operator=ComparisonOperator.LessThan,
    ),
    # Structural check using AssertionTask alongside agent tasks
    AssertionTask(
        id="context_not_empty",
        context_path="response",
        operator=ComparisonOperator.IsNotEmpty,
        expected_value=True,
    ),
]

dataset = EvalDataset(records=records, tasks=tasks)
results = dataset.evaluate()
results.as_table()
```

### Online Evaluation

Attach `AgentAssertionTask` to a `AgentEvalProfile` for real-time monitoring. The server samples `EvalRecord` objects at the configured `sample_ratio` and evaluates the tasks asynchronously.

```python
from scouter.evaluate import AgentAssertion, AgentAssertionTask, AgentEvalProfile, EvalRecord
from scouter import AgentEvalConfig, GrpcConfig
from scouter.queue import ScouterQueue
from scouter.alert import ConsoleDispatchConfig
from scouter.types import CommonCrons

profile = AgentEvalProfile(
    config=AgentEvalConfig(
        space="production",
        name="search-agent",
        version="1.0.0",
        sample_ratio=0.2,  # Evaluate 20% of requests
        alert_config=ConsoleDispatchConfig(),
        schedule=CommonCrons.EveryHour,
    ),
    tasks=[
        AgentAssertionTask(
            id="search_called",
            assertion=AgentAssertion.tool_called("web_search"),
            expected_value=True,
            operator=ComparisonOperator.Equals,
            description="Search tool must be invoked",
        ),
        AgentAssertionTask(
            id="no_dangerous_tools",
            assertion=AgentAssertion.tool_not_called("delete_all_records"),
            expected_value=True,
            operator=ComparisonOperator.Equals,
        ),
        AgentAssertionTask(
            id="output_token_budget",
            assertion=AgentAssertion.response_output_tokens(),
            expected_value=2000,
            operator=ComparisonOperator.LessThan,
        ),
    ],
)

# Register the profile (typically done once at deployment time)
client.register_profile(profile, set_active=True)

# At inference time, insert records into the queue
queue = ScouterQueue.from_path(
    path={"search-agent": profile},
    transport_config=GrpcConfig(),
)

# After each agent invocation, record the raw response
raw_response = call_my_agent(user_query)

queue.insert(
    "search-agent",
    EvalRecord(context={"response": raw_response}),
)
```

The server picks up sampled records, runs the tasks, and triggers alerts based on the profile's `alert_config` and `schedule`.

---

## Task Interplay

The three task types are complementary and designed to work together in layered evaluation pipelines.

```
AssertionTask        — validates what the model returned (context values, structure, thresholds)
LLMJudgeTask         — evaluates quality of what the model returned (semantics, reasoning, tone)
TraceAssertionTask   — validates how the model produced it (span sequence, retries, latency, tokens)
```

A typical evaluation pipeline uses all three layers:

```python
from scouter.evaluate import (
    AssertionTask,
    LLMJudgeTask,
    TraceAssertionTask,
    TraceAssertion,
    SpanFilter,
    AggregationType,
    ComparisonOperator,
)

tasks = [
    # Layer 1 — structural gate: only proceed if the response has the required shape
    AssertionTask(
        id="has_required_fields",
        context_path="response",
        operator=ComparisonOperator.ContainsAll,
        expected_value=["answer", "sources", "confidence"],
        condition=True,  # Gate: skip all downstream if structure is wrong
    ),

    # Layer 2 — trace gate: only proceed if the agent executed in the correct order
    TraceAssertionTask(
        id="correct_execution_order",
        assertion=TraceAssertion.span_sequence(["retrieve", "rerank", "generate"]),
        operator=ComparisonOperator.Equals,
        expected_value=True,
        condition=True,  # Gate: skip quality check if pipeline ran out of order
    ),

    # Layer 3 — quality: run LLM judge only if structure and execution are valid
    LLMJudgeTask(
        id="relevance_check",
        prompt=relevance_prompt,
        expected_value=7,
        context_path="score",
        operator=ComparisonOperator.GreaterThanOrEqual,
        depends_on=["has_required_fields", "correct_execution_order"],
        description="Check answer relevance after structural and execution validation",
    ),

    # Layer 3 — resource check: runs alongside quality check, independent
    TraceAssertionTask(
        id="token_budget",
        assertion=TraceAssertion.span_aggregation(
            filter=SpanFilter.by_name_pattern(r"llm\..*"),
            attribute_key="token_count",
            aggregation=AggregationType.Sum,
        ),
        operator=ComparisonOperator.LessThan,
        expected_value=5000,
        description="Ensure total token usage stays within budget",
    ),
]
```

**When to use each task type:**

| Question | Task type |
|----------|-----------|
| Did the response contain the right fields? | `AssertionTask` |
| Is the response numerically within threshold? | `AssertionTask` |
| Did the agent call the right tools in order? | `AgentAssertionTask` |
| Was a specific tool called with the right arguments? | `AgentAssertionTask` |
| Did total response tokens exceed the budget? | `AgentAssertionTask` |
| Did the response finish with the expected stop reason? | `AgentAssertionTask` |
| Did the agent retry more than N times? | `TraceAssertionTask` |
| Did total latency exceed the SLA? | `TraceAssertionTask` |
| Did total token usage across spans exceed the budget? | `TraceAssertionTask` |
| Is the response semantically relevant? | `LLMJudgeTask` |
| Does the response contain hallucinations? | `LLMJudgeTask` |
| Is the tone appropriate for the audience? | `LLMJudgeTask` |

---

## ComparisonOperator

`ComparisonOperator` defines how task outputs are compared against expected values. Operators are grouped into categories for different data types and validation needs.

### Numeric Comparisons

| Operator | Symbol | Description | Example |
|----------|--------|-------------|---------|
| `Equals` | `==` | Exact equality | `score == 5` |
| `NotEqual` | `!=` | Not equal | `status != "error"` |
| `GreaterThan` | `>` | Greater than | `confidence > 0.9` |
| `GreaterThanOrEqual` | `>=` | Greater than or equal | `score >= 7` |
| `LessThan` | `<` | Less than | `latency < 100` |
| `LessThanOrEqual` | `<=` | Less than or equal | `error_rate <= 0.01` |
| `InRange` | `[min, max]` | Within numeric range | `temperature in [20, 25]` |
| `NotInRange` | `outside [min, max]` | Outside numeric range | `outlier not in [0, 100]` |
| `ApproximatelyEquals` | `≈` | Within tolerance | `value ≈ 3.14 (±0.01)` |
| `IsPositive` | `> 0` | Is positive number | `growth > 0` |
| `IsNegative` | `< 0` | Is negative number | `loss < 0` |
| `IsZero` | `== 0` | Is exactly zero | `balance == 0` |

### String Comparisons

| Operator | Description | Example |
|----------|-------------|---------|
| `Contains` | Contains substring | `response contains "Paris"` |
| `NotContains` | Does not contain substring | `text not contains "error"` |
| `StartsWith` | Starts with prefix | `code starts with "def"` |
| `EndsWith` | Ends with suffix | `filename ends with ".json"` |
| `Matches` | Matches regex pattern | `email matches r".*@.*\.com"` |
| `MatchesRegex` | Matches regex (alias) | Same as `Matches` |
| `ContainsWord` | Contains specific word | `sentence contains word "hello"` |
| `IsAlphabetic` | Only alphabetic chars | `"abc" is alphabetic` |
| `IsAlphanumeric` | Alphanumeric chars only | `"abc123" is alphanumeric` |
| `IsLowerCase` | All lowercase | `"hello" is lowercase` |
| `IsUpperCase` | All uppercase | `"HELLO" is uppercase` |

### Collection Comparisons

| Operator | Description | Example |
|----------|-------------|---------|
| `ContainsAll` | Contains all elements | `tags contains all ["valid", "reviewed"]` |
| `ContainsAny` | Contains any element | `categories contains any ["tech", "science"]` |
| `ContainsNone` | Contains none of elements | `forbidden contains none ["spam", "abuse"]` |
| `HasUniqueItems` | All items are unique | `ids has unique items` |
| `IsEmpty` | Collection is empty | `errors is empty` |
| `IsNotEmpty` | Collection has items | `results is not empty` |

### Length Validations

| Operator | Description | Example |
|----------|-------------|---------|
| `HasLengthEqual` | Exact length | `response has length == 100` |
| `HasLengthGreaterThan` | Length greater than | `text has length > 50` |
| `HasLengthLessThan` | Length less than | `summary has length < 200` |
| `HasLengthGreaterThanOrEqual` | Length >= value | `content has length >= 10` |
| `HasLengthLessThanOrEqual` | Length <= value | `title has length <= 60` |

### Type Validations

| Operator | Description | Example |
|----------|-------------|---------|
| `IsNumeric` | Is numeric type | `value is numeric` |
| `IsString` | Is string type | `name is string` |
| `IsBoolean` | Is boolean type | `flag is boolean` |
| `IsNull` | Is null/None | `optional_field is null` |
| `IsArray` | Is array/list | `items is array` |
| `IsObject` | Is object/dict | `metadata is object` |

### Format Validations

| Operator | Description | Example |
|----------|-------------|---------|
| `IsEmail` | Valid email format | `"user@example.com" is email` |
| `IsUrl` | Valid URL format | `"https://example.com" is url` |
| `IsUuid` | Valid UUID format | `"550e8400-e29b-41d4-a716..." is uuid` |
| `IsIso8601` | Valid ISO 8601 date | `"2024-01-08T12:00:00Z" is iso8601` |
| `IsJson` | Valid JSON format | `'{"key": "value"}' is json` |

## Usage in Evaluation Workflows

### Offline Evaluation

Tasks are combined in a `EvalDataset` for batch evaluation:

```python
from scouter.evaluate import EvalDataset

dataset = EvalDataset(
    records=[record1, record2, record3],
    tasks=[
        AssertionTask(id="length_check", ...),
        LLMJudgeTask(id="quality_check", ...),
        AssertionTask(id="score_threshold", depends_on=["quality_check"], ...)
    ]
)

results = dataset.evaluate()
results.as_table()
```

### Online Monitoring

Tasks are included in a `AgentEvalProfile` for real-time monitoring:

```python
from scouter.evaluate import AgentEvalProfile
from scouter import AgentEvalConfig

profile = AgentEvalProfile(
    config=AgentEvalConfig(
        space="production",
        name="chatbot",
        sample_ratio=0.1  # Evaluate 10% of traffic
    ),
    tasks=[
        AssertionTask(id="not_empty", ...),
        LLMJudgeTask(id="relevance", depends_on=["not_empty"], ...),
        AssertionTask(id="final_check", depends_on=["relevance"], ...)
    ]
)

# Register for production monitoring
client.register_profile(profile, set_active=True)
```

## Best Practices

### When to Use AssertionTask

- **Structure validation**: Check required fields exist
- **Threshold checks**: Validate numeric ranges or limits
- **Fast pre-filtering**: Gate expensive LLM judges with quick checks
- **Format validation**: Email, URL, JSON format checks
- **Type checking**: Ensure correct data types

### When to Use LLMJudgeTask

- **Semantic evaluation**: Similarity, relevance, coherence
- **Quality assessment**: Subjective criteria like helpfulness or clarity
- **Complex reasoning**: Multi-factor evaluations requiring judgment
- **Factuality checking**: Hallucination detection, source verification
- **Style/tone analysis**: Appropriate language for audience

### When to Use AgentAssertionTask

- **Tool call verification**: Check that specific tools were (or were not) called
- **Argument validation**: Assert tool arguments match expected values
- **Call sequence**: Verify tools were called in the correct order
- **Response content**: Check text content of agent responses
- **Token budgets**: Assert input, output, or total token counts from the response object
- **Vendor-specific fields**: Use `response_field(path)` for fields not covered by named variants

### When to Use TraceAssertionTask

- **Execution order**: Verify spans ran in the correct sequence
- **Retry enforcement**: Limit retry attempts to prevent runaway agents
- **Token budgets**: Sum token usage across all LLM calls in a trace
- **Latency SLAs**: Assert total trace or individual span duration
- **Error detection**: Count spans with ERROR status
- **Model verification**: Check which model was used in a specific span

### Optimization Tips

1. **Assertions first**: Use `AssertionTask` for fast structural validation before expensive LLM calls
2. **Trace gates early**: Use `TraceAssertionTask` with `condition=True` to skip quality evaluation when execution was invalid
3. **Conditional gates**: Use `condition=True` to prevent unnecessary downstream evaluation
4. **Minimize dependencies**: Only depend on tasks whose outputs you actually need
5. **Appropriate operators**: Choose the most specific operator for your validation
6. **Clear field paths**: Use explicit paths to access nested data (`"response.metadata.score"`)

### Error Handling

- **AssertionTask**: Fails immediately on type mismatch or missing fields
- **AgentAssertionTask**: Fails if the response cannot be deserialized or the assertion path does not resolve
- **TraceAssertionTask**: Fails if no spans match the filter (for span-level assertions)
- **LLMJudgeTask**: Respects `max_retries` for transient failures (network, rate limits)
- **Conditional tasks**: Failed conditions skip dependent tasks without failing the workflow
- **Field paths**: Invalid paths cause task failure with clear error messages

## Next Steps

Now that you understand evaluation tasks, explore how to build complete GenAI evaluation workflows:

- [Offline Evaluation](/scouter/docs/monitoring/genai/offline-evaluation/) - Batch evaluation with complex task chains
- [Online Monitoring](/scouter/docs/monitoring/genai/online-evaluation/) - Production monitoring setup
- [Distributed Tracing](/scouter/docs/tracing/overview/) - Capture trace data that `TraceAssertionTask` evaluates
