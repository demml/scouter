# Task Parsing Documentation

This directory contains examples and documentation for parsing evaluation tasks from YAML and JSON files in Scouter.

## Overview

Scouter provides a flexible task parsing system that allows you to define evaluation workflows declaratively using YAML or JSON files. This supports three types of tasks:

1. **AssertionTask**: Direct assertions on field values
2. **LLMJudgeTask**: LLM-based evaluation tasks
3. **TraceAssertionTask**: Assertions on OpenTelemetry trace data

## Quick Start

### Python API

```python
from scouter import load_tasks_from_file, load_task_from_string

# Load multiple tasks from a file
tasks = load_tasks_from_file("path/to/tasks.yaml")

# Load a single task from a string
task = load_task_from_string(yaml_content, "yaml")
```

### Available Functions

- `load_task_from_file(path: str)` - Load a single task from a YAML/JSON file
- `load_tasks_from_file(path: str)` - Load multiple tasks from a YAML/JSON file
- `load_task_from_string(content: str, format: str)` - Load a single task from a string
- `load_tasks_from_string(content: str, format: str)` - Load multiple tasks from a string

## Task Structure

### Common Fields

All tasks share these common fields:

- `task_type`: (Required) One of: `Assertion`, `LLMJudge`, or `TraceAssertion`
- `id`: (Required) Unique identifier for the task
- `operator`: (Required) Comparison operator (see Operators section)
- `expected_value`: (Required) Expected value for comparison
- `description`: (Optional) Human-readable description
- `depends_on`: (Optional) Array of task IDs this task depends on
- `condition`: (Optional) Boolean, defaults to `false`

### 1. AssertionTask

Direct assertions on field values extracted from context data.

#### Required Fields
- `field_path`: Dot-notation path to extract value from context (e.g., "user.age", "response.data.items")

#### YAML Example
```yaml
task_type: Assertion
id: check_user_age
field_path: user.age
operator: GreaterThan
expected_value: 18
description: Verify user is an adult
depends_on: []
condition: false
```

#### JSON Example
```json
{
  "task_type": "Assertion",
  "id": "check_user_age",
  "field_path": "user.age",
  "operator": "GreaterThan",
  "expected_value": 18,
  "description": "Verify user is an adult",
  "depends_on": [],
  "condition": false
}
```

### 2. LLMJudgeTask

Uses an LLM to evaluate content and compare the result.

#### Required Fields
- `prompt`: Prompt configuration (can be inline or a path reference)
- `field_path`: (Optional) Path to extract value from context for evaluation

#### Additional Fields
- `max_retries`: (Optional) Maximum retry attempts, defaults to 3

#### Prompt Configuration

The `prompt` field can be specified in two ways:

**Option 1: Path reference**
```yaml
prompt:
  path: "./prompts/sentiment_judge.json"
```

**Option 2: Inline prompt object**
```yaml
prompt:
  model: gpt-4o-mini
  provider: OpenAI
  version: "1.0.0"
  parameters: ["text"]
  response_type: Text
  request:
    # ... full prompt configuration
```

#### YAML Example
```yaml
task_type: LLMJudge
id: sentiment_judge
field_path: response.text
operator: Equals
expected_value: Positive
description: Judge sentiment using LLM
depends_on: []
max_retries: 3
condition: false
prompt:
  path: "./prompts/sentiment_judge.json"
```

#### JSON Example
```json
{
  "task_type": "LLMJudge",
  "id": "sentiment_judge",
  "field_path": "response.text",
  "operator": "Equals",
  "expected_value": "Positive",
  "description": "Judge sentiment using LLM",
  "depends_on": [],
  "max_retries": 3,
  "condition": false,
  "prompt": {
    "path": "./prompts/sentiment_judge.json"
  }
}
```

### 3. TraceAssertionTask

Assertions on OpenTelemetry trace data.

#### Required Fields
- `assertion`: Trace assertion configuration specifying what to check

#### Assertion Types

**TraceDuration**: Check total trace duration
```yaml
assertion:
  TraceDuration: {}
```

**TraceSpanCount**: Count total spans
```yaml
assertion:
  TraceSpanCount: {}
```

**TraceErrorCount**: Count error spans
```yaml
assertion:
  TraceErrorCount: {}
```

**SpanSequence**: Check span order
```yaml
assertion:
  SpanSequence:
    span_names:
      - call_tool
      - run_agent
      - double_check
```

**SpanCount**: Count spans matching filter
```yaml
assertion:
  SpanCount:
    filter:
      ByName:
        name: retry_operation
```

**SpanExists**: Check if span exists
```yaml
assertion:
  SpanExists:
    filter:
      WithStatus:
        status: Error
```

**SpanAttribute**: Get attribute value
```yaml
assertion:
  SpanAttribute:
    filter:
      ByName:
        name: llm.generate
    attribute_key: model
```

**SpanDuration**: Get span duration
```yaml
assertion:
  SpanDuration:
    filter:
      ByName:
        name: database_query
```

**SpanAggregation**: Aggregate numeric attributes
```yaml
assertion:
  SpanAggregation:
    filter:
      ByName:
        name: api_call
    attribute_key: duration_ms
    aggregation: Average
```

#### Span Filters

Filters can be combined using `And` and `Or` logic:

```yaml
filter:
  And:
    filters:
      - WithStatus:
          status: Error
      - WithAttribute:
          key: error.type
```

Available filter types:
- `ByName`: Match exact name
- `ByNamePattern`: Match regex pattern
- `WithAttribute`: Has attribute key
- `WithAttributeValue`: Has key-value pair
- `WithStatus`: Match status (Ok, Error, Unset)
- `WithDuration`: Duration in range
- `And`: Combine filters (all must match)
- `Or`: Combine filters (any must match)

#### YAML Example
```yaml
task_type: TraceAssertion
id: verify_agent_workflow
operator: SequenceMatches
expected_value: true
description: Verify agent workflow execution order
depends_on: []
condition: false
assertion:
  SpanSequence:
    span_names:
      - call_tool
      - run_agent
      - double_check
```

## Comparison Operators

### Basic Comparisons
- `Equals`: Exact equality
- `NotEqual`: Not equal
- `GreaterThan`: Numeric greater than
- `GreaterThanOrEqual`: Numeric greater than or equal
- `LessThan`: Numeric less than
- `LessThanOrEqual`: Numeric less than or equal

### String Operators
- `Contains`: String contains substring
- `NotContains`: String doesn't contain substring
- `StartsWith`: String starts with prefix
- `EndsWith`: String ends with suffix
- `Matches`: Regex pattern match
- `IsAlphabetic`: Only alphabetic characters
- `IsAlphanumeric`: Only alphanumeric characters
- `IsLowerCase`: All lowercase
- `IsUpperCase`: All uppercase
- `ContainsWord`: Contains specific word

### Length Operators
- `HasLengthEqual`: Length equals value
- `HasLengthGreaterThan`: Length greater than
- `HasLengthLessThan`: Length less than
- `HasLengthGreaterThanOrEqual`: Length ≥ value
- `HasLengthLessThanOrEqual`: Length ≤ value

### Type Validation
- `IsNumeric`: Value is numeric
- `IsString`: Value is string
- `IsBoolean`: Value is boolean
- `IsNull`: Value is null
- `IsArray`: Value is array
- `IsObject`: Value is object

### Format Validation
- `IsEmail`: Valid email format
- `IsUrl`: Valid URL format
- `IsUuid`: Valid UUID format
- `IsIso8601`: Valid ISO 8601 date
- `IsJson`: Valid JSON string
- `MatchesRegex`: Matches regex pattern

### Numeric Operators
- `InRange`: Value in numeric range
- `NotInRange`: Value outside range
- `IsPositive`: Positive number
- `IsNegative`: Negative number
- `IsZero`: Exactly zero
- `ApproximatelyEquals`: Nearly equal (with tolerance)

### Collection Operators
- `SequenceMatches`: Array matches sequence
- `ContainsAll`: Contains all items
- `ContainsAny`: Contains any item
- `ContainsNone`: Contains no items
- `IsEmpty`: Collection is empty
- `IsNotEmpty`: Collection is not empty
- `HasUniqueItems`: All items unique

## Task Dependencies

Tasks can depend on other tasks using the `depends_on` field:

```yaml
tasks:
  - task_type: Assertion
    id: validate_email
    field_path: user.email
    operator: IsEmail
    expected_value: true
    depends_on: []
  
  - task_type: Assertion
    id: check_email_domain
    field_path: user.email
    operator: EndsWith
    expected_value: "@company.com"
    depends_on:
      - validate_email  # Only runs if validate_email passes
```

## Conditional Execution

Set `condition: true` to make a task only execute when all dependencies pass:

```yaml
task_type: Assertion
id: final_check
field_path: status
operator: Equals
expected_value: success
depends_on:
  - check_1
  - check_2
condition: true  # Only runs if check_1 AND check_2 pass
```

## Complete Example

See [eval_tasks_example.yaml](./eval_tasks_example.yaml) and [eval_tasks_example.json](./eval_tasks_example.json) for comprehensive examples.

## Python Usage Examples

See [task_parsing_example.py](./task_parsing_example.py) for complete Python usage examples including:

- Loading tasks from files
- Loading tasks from strings
- Working with different task types
- Handling task dependencies
- Complex trace assertions

## Error Handling

The parsing functions will raise `TypeError` exceptions for:

- Invalid file paths
- Malformed YAML/JSON
- Missing required fields
- Invalid operator names
- Type mismatches
- Invalid prompt paths (for LLMJudgeTask)

Example error handling:

```python
from scouter import load_tasks_from_file, TypeError

try:
    tasks = load_tasks_from_file("tasks.yaml")
except TypeError as e:
    print(f"Failed to load tasks: {e}")
```

## Best Practices

1. **Use meaningful IDs**: Task IDs should be descriptive and unique
2. **Add descriptions**: Always include descriptions for maintainability
3. **Organize dependencies**: Structure tasks in logical groups
4. **Validate early**: Use basic assertions before complex LLM judges
5. **Reuse prompts**: Store prompts in separate files and reference by path
6. **Test incrementally**: Start with simple tasks and build complexity
7. **Version control**: Keep task definitions in version control
8. **Document operators**: Comment on why specific operators are used

## Integration with Evaluation Workflows

Loaded tasks can be used directly with Scouter's evaluation engines:

```python
from scouter import load_tasks_from_file

# Load tasks
tasks = load_tasks_from_file("evaluation_tasks.yaml")

# Use in your evaluation workflow
# (See main Scouter documentation for evaluation engine usage)
```
