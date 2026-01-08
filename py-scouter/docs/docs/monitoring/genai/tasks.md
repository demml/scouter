# Evaluation Tasks: Building Blocks for GenAI Evaluation

Evaluation tasks are the core building blocks of Scouter's GenAI evaluation framework. They define what to evaluate, how to evaluate it, and what conditions must be met for an evaluation to pass. Tasks can be combined with dependencies to create sophisticated evaluation workflows that handle complex, multi-stage assessments.

## Task Types

Scouter provides two types of evaluation tasks:

| Task Type | Evaluation Method | Use Cases | Cost/Latency |
|-----------|------------------|-----------|--------------|
| **AssertionTask** | Deterministic rule-based validation | Structure validation, threshold checks, pattern matching | Zero cost, minimal latency |
| **LLMJudgeTask** | LLM-powered reasoning | Semantic similarity, quality assessment, complex criteria | Additional LLM call cost and latency |

Both task types support:
- **Dependencies**: Chain tasks to build on previous results
- **Conditional execution**: Use tasks as gates to control downstream evaluation
- **Field path extraction**: Access nested values in context or upstream outputs

## AssertionTask

`AssertionTask` performs fast, deterministic validation without requiring additional LLM calls. Assertions evaluate values from the evaluation context against expected conditions using comparison operators.

### Parameters

```python
AssertionTask(
    id: str,
    expected_value: Any,
    operator: ComparisonOperator,
    field_path: Optional[str] = None,
    description: Optional[str] = None,
    depends_on: Optional[List[str]] = None,
    condition: bool = False,
)
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id` | `str` | Yes | Unique identifier (converted to lowercase). Used in dependencies and results. |
| `expected_value` | `Any` | Yes | Value to compare against. Must be JSON-serializable (str, int, float, bool, list, dict, None). |
| `operator` | `ComparisonOperator` | Yes | Comparison operator to use for evaluation. |
| `field_path` | `str` | No | Dot-notation path to extract value from context (e.g., `"response.confidence"`). If `None`, uses entire context. |
| `description` | `str` | No | Human-readable description for understanding results. |
| `depends_on` | `List[str]` | No | Task IDs that must complete before this task executes. Outputs from dependencies are added to context. |
| `condition` | `bool` | No | If `True`, acts as conditional gate. Failed conditions skip dependent tasks and exclude this task from final results. |

### Common Patterns

**Threshold Validation:**

```python
AssertionTask(
    id="confidence_check",
    field_path="model_output.confidence",
    operator=ComparisonOperator.GreaterThanOrEqual,
    expected_value=0.85,
    description="Require confidence >= 85%"
)
```

**Structure Validation:**

```python
AssertionTask(
    id="has_required_fields",
    field_path="response",
    operator=ComparisonOperator.ContainsAll,
    expected_value=["answer", "sources", "confidence"],
    description="Ensure response has all required fields"
)
```

**Conditional Gate:**

```python
AssertionTask(
    id="is_production_ready",
    field_path="metadata.environment",
    operator=ComparisonOperator.Equals,
    expected_value="production",
    condition=True,  # Downstream tasks only run if this passes
    description="Gate for production-only evaluations"
)
```

**Dependent Assertion:**

```python
AssertionTask(
    id="technical_score_high",
    field_path="expert_validation.technical_score",
    operator=ComparisonOperator.GreaterThan,
    expected_value=8,
    depends_on=["expert_validation"],  # Access upstream LLMJudgeTask output
    description="Technical score must exceed 8"
)
```

## LLMJudgeTask

`LLMJudgeTask` uses an additional LLM call to evaluate responses based on sophisticated criteria requiring reasoning, context understanding, or subjective judgment. LLM judges are ideal for evaluations that cannot be captured by deterministic rules.

### Parameters

```python
LLMJudgeTask(
    id: str,
    prompt: Prompt,
    expected_value: Any,
    field_path: Optional[str],
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
| `field_path` | `str` | No | Dot-notation path to extract from LLM response (e.g., `"score"`). If `None`, uses entire response. |
| `operator` | `ComparisonOperator` | Yes | Comparison operator for evaluating LLM response against expected value. |
| `description` | `str` | No | Human-readable description of evaluation purpose. |
| `depends_on` | `List[str]` | No | Task IDs that must complete first. Dependency outputs are added to context and available in prompt. |
| `max_retries` | `int` | No | Maximum retry attempts for LLM call failures (network errors, rate limits). Defaults to 3. |
| `condition` | `bool` | No | If `True`, acts as conditional gate for dependent tasks. |

### Common Patterns

**Basic Quality Assessment:**

```python
from scouter.genai import Score

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
    field_path="score",
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
    field_path="score",
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
    field_path="score",
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
    field_path="score",
    operator=ComparisonOperator.GreaterThanOrEqual
)

# Stage 2: Factuality (depends on relevance)
factuality_task = LLMJudgeTask(
    id="factuality",
    prompt=factuality_prompt,
    expected_value=8,
    field_path="score",
    operator=ComparisonOperator.GreaterThanOrEqual,
    depends_on=["relevance"],
    description="Factuality check after relevance validation"
)
```

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

Tasks are combined in a `GenAIEvalDataset` for batch evaluation:

```python
from scouter.evaluate import GenAIEvalDataset

dataset = GenAIEvalDataset(
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

Tasks are included in a `GenAIEvalProfile` for real-time monitoring:

```python
from scouter.evaluate import GenAIEvalProfile
from scouter import GenAIDriftConfig

profile = GenAIEvalProfile(
    config=GenAIDriftConfig(
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

### Optimization Tips

1. **Assertions first**: Use `AssertionTask` for fast validation before expensive LLM calls
2. **Conditional gates**: Use `condition=True` to prevent unnecessary downstream evaluation
3. **Minimize dependencies**: Only depend on tasks whose outputs you actually need
4. **Appropriate operators**: Choose the most specific operator for your validation
5. **Clear field paths**: Use explicit paths to access nested data (`"response.metadata.score"`)

### Error Handling

- **AssertionTask**: Fails immediately on type mismatch or missing fields
- **LLMJudgeTask**: Respects `max_retries` for transient failures (network, rate limits)
- **Conditional tasks**: Failed conditions skip dependent tasks without failing the workflow
- **Field paths**: Invalid paths cause task failure with clear error messages

## Examples

For complete examples demonstrating task usage in real-world scenarios:

- [Offline Evaluation](/scouter/docs/monitoring/genai/offline-evaluation/) - Batch evaluation with complex task chains
- [Online Monitoring](/scouter/docs/monitoring/genai/online-evaluation/) - Production monitoring setup
