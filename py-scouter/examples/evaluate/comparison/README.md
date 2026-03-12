# Evaluation Comparison Examples

Demonstrates comparing two evaluation runs to detect regressions or measure improvement. The pattern is: run evaluation on a baseline, update the contexts, re-run, then call `compare_to()`.

## How comparison works

1. Assign stable `id` values to `EvalRecord` objects
2. Run baseline evaluation → `baseline_results`
3. Build a new dataset using `with_updated_contexts_by_id(context_map)` — only the fields you changed need to be in the map; everything else carries over
4. Run improved evaluation → `improved_results`
5. Call `improved_results.compare_to(baseline=baseline_results, regression_threshold=0.05)`

The comparison reports which record workflows regressed, improved, or were unchanged.

```python
# Partial context update — only replace the model output, keep the inputs
context_map = {
    "record_0": {"agent_response": new_response_0},
    "record_1": {"agent_response": new_response_1},
}
improved_dataset = baseline_dataset.with_updated_contexts_by_id(context_map)
improved_results = improved_dataset.evaluate()
comparison = improved_results.compare_to(baseline=baseline_results, regression_threshold=0.05)
```

## Examples

### `product_categorization.py` — model accuracy and performance comparison

Compares a baseline product classifier (some mislabelled categories, lower confidence, slower) against an improved version.

```bash
cd py-scouter
uv run python examples/evaluate/comparison/product_categorization.py
```

**What it shows:**
- Assigning record `id` parameters for stable cross-run tracking
- `AssertionTask` with `${ground_truth}` template variable for dynamic expected values
- Checking confidence, processing time, feature count, and keyword presence
- Using `context_map` to replace only `prediction` while keeping `ground_truth` unchanged
- Reading `comparison.regressed_workflows` and `comparison.improved_workflows`

**Baseline issues the improved model fixes:**
- Two mislabelled categories (clothing→accessories, books→media)
- Confidence below the 0.85 threshold
- Processing time above the 50ms threshold

---

### `retail_helper_agent.py` — customer support agent quality comparison

Compares a dismissive baseline agent against an improved empathetic one, using LLM-based quality, accuracy, and empathy evaluation.

```bash
export OPENAI_API_KEY=<your-key>   # or ANTHROPIC_API_KEY / GOOGLE_API_KEY
cd py-scouter
uv run python examples/evaluate/comparison/retail_helper_agent.py
```

**What it shows:**
- LLM judge tasks with domain-expert prompts (`ResponseQuality`, `TechnicalAccuracy`, `EmpathyAssessment`)
- Score threshold `AssertionTask` tasks that `depends_on` the corresponding judge task output
- Partial context update: only `agent_response` changes between runs; `customer_query`, `customer_context`, and `urgency` stay the same
- `regression_threshold=0.05` — a 5% tolerance before a workflow is flagged as regressed
- Interpreting comparison output: improved vs. regressed vs. no change

## Key API

```python
# Create baseline dataset
baseline_dataset = EvalDataset(
    records=[EvalRecord(id="interaction_0", context={...}), ...],
    tasks=[...],
)
baseline_results = baseline_dataset.evaluate()

# Update only what changed
improved_dataset = baseline_dataset.with_updated_contexts_by_id({
    "interaction_0": {"agent_response": better_response},
})
improved_results = improved_dataset.evaluate()

# Compare
comparison = improved_results.compare_to(
    baseline=baseline_results,
    regression_threshold=0.05,
)
print(f"Improved: {comparison.improved_workflows}")
print(f"Regressed: {comparison.regressed_workflows}")
```

## When to use this pattern

- Evaluating a model version before promotion to production
- Measuring the effect of a prompt change
- Regression testing after fine-tuning
- A/B comparison of two retrieval strategies or agent configurations
