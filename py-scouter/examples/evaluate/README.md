# Evaluate Examples

Offline batch evaluation using `EvalDataset`. These examples run without a live Scouter server — evaluation executes locally and results are printed to stdout.

All examples require an LLM provider API key for `LLMJudgeTask` examples. `AssertionTask`-only examples run without one.

## How offline evaluation works

1. Define evaluation tasks (`AssertionTask` or `LLMJudgeTask`)
2. Create `EvalRecord` objects containing the data to evaluate
3. Build a `EvalDataset` with the records and tasks
4. Call `dataset.evaluate()` — tasks run, results are returned
5. Inspect results via `results.as_table()` or `results.as_dataframe()`

## Examples

### `customer_support.py` — minimal working example

The simplest example: one LLM judge task and three assertion checks on a customer support response.

```bash
export OPENAI_API_KEY=<your-key>   # or ANTHROPIC_API_KEY / GOOGLE_API_KEY
cd py-scouter
uv run python examples/evaluate/customer_support.py
```

**What it shows:**
- Defining a `LLMJudgeTask` with a custom Pydantic output type (`ResponseQuality`)
- Using `AssertionTask` to check boolean fields and numeric thresholds from a `EvalRecord` context
- `context_path` to extract nested fields (e.g. `response.confidence_score`)
- Running evaluation and printing a results table

---

### `reformulation_relevance.py` — embeddings and similarity

Evaluates a query reformulation pipeline. Adds OpenAI embedding-based similarity computation on top of standard LLM judge tasks.

```bash
export OPENAI_API_KEY=<your-key>
cd py-scouter
uv run python examples/evaluate/reformualtion_relevance.py
```

**What it shows:**
- Multiple `LLMJudgeTask` tasks with `Score` output type (score + reason)
- Template variables in prompts (`${user_query}`, `${reformulated_query}`, `${answer}`)
- `AssertionTask` with `IsNotEmpty` to guard against empty outputs
- `EvaluationConfig` with `OpenAIEmbeddingConfig` for semantic similarity metrics
- `compute_similarity=True` and `compute_histograms=True` in `dataset.evaluate()`

---

### `query_reformulation_safety.py` — multi-step safety check

Evaluates a query reformulation for quality and safety using two independent LLM judge tasks.

```bash
export GOOGLE_API_KEY=<your-key>
cd py-scouter
uv run python examples/evaluate/query_reformulation_safety.py
```

**What it shows:**
- Using `Agent` to generate content and then evaluate it in the same script
- Two parallel judge tasks: quality scoring and harmfulness detection
- Custom output type `IsHarmful` (boolean + reason)
- `ComparisonOperator.Equals` to assert `is_harmful == False`

---

### `recipe.py` — conditional task gates and nested context paths

Evaluates LLM-generated recipes with conditional logic: downstream tasks only run if an upstream gate passes.

```bash
export GOOGLE_API_KEY=<your-key>
cd py-scouter
uv run python examples/evaluate/recipe.py
```

**What it shows:**
- `condition=True` on an `AssertionTask` to create a gate — downstream tasks are skipped if it fails
- `depends_on` to wire task execution order
- Nested `context_path` access (e.g. `recipe.rating.score`, `recipe.ingredients`)
- `ComparisonOperator.InRange` for numeric range checks
- `Agent` generating structured Pydantic output used directly as evaluation context
- Mixed records — some with optional fields present, some without — to exercise conditional paths

---

### `retail_question.py` — category-based routing with cascading dependencies

Evaluates a retail support agent across three product categories (kitchen, bath, outdoor), using a category classifier to route records to domain-specific expert validation tasks.

```bash
export GOOGLE_API_KEY=<your-key>
cd py-scouter
uv run python examples/evaluate/retail_question.py
```

**What it shows:**
- Category classification as an `AssertionTask` gate (`condition=True`) routing to domain-specific tasks
- Three independent expert validation chains (`LLMJudgeTask` → score threshold `AssertionTask`)
- Deep `depends_on` chains: `expert_validation` depends on category gate; score check depends on expert result
- Using `model.model_dump_json()` to pass Pydantic objects into `EvalRecord` context
- Score thresholds per domain (technical accuracy, installation guidance, durability)

---

### `comparison/` — baseline vs. improved evaluation

See the [`comparison/` README](comparison/README.md) for examples that compare two evaluation runs to detect regressions or measure improvements.

## Task reference

### AssertionTask

Deterministic checks. No LLM required.

```python
AssertionTask(
    task_id="check_score",
    context_path="result.score",        # dot-notation field extraction
    operator=ComparisonOperator.GreaterThanOrEqual,
    expected=7,
    condition=False,                    # set True to gate downstream tasks
    depends_on=["upstream_task_id"],    # optional: access upstream outputs
)
```

### LLMJudgeTask

Semantic evaluation via LLM with structured Pydantic output.

```python
class MyOutput(BaseModel):
    score: int
    reason: str

LLMJudgeTask(
    task_id="quality_check",
    prompt=Prompt(message="Rate this on a scale of 1-5: ${response}"),
    output_type=MyOutput,
    context_path="score",               # field to extract for downstream tasks
    depends_on=[],
)
```

### ComparisonOperator selection

| Use case | Operator |
|----------|---------|
| Exact match | `Equals` |
| Numeric bounds | `GreaterThan`, `LessThanOrEqual`, `InRange` |
| String content | `Contains`, `StartsWith`, `Matches` (regex) |
| Emptiness | `IsEmpty`, `IsNotEmpty` |
| JSON validity | `IsJson` |
| Collection length | `HasLength`, `HasLengthGreaterThan` |
| Boolean | `IsTrue`, `IsFalse` |
