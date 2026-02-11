"""
Example usage of the task parsing functionality in Scouter.

This demonstrates how to load evaluation tasks from YAML/JSON files
and use them in your evaluation workflows.
"""

from scouter import (
    load_task_from_file,
    load_tasks_from_file,
    load_task_from_string,
    load_tasks_from_string,
    AssertionTask,
    LLMJudgeTask,
    TraceAssertionTask,
)


def example_load_single_task():
    """Load a single task from a YAML/JSON string."""
    
    # Example 1: Load AssertionTask from YAML string
    yaml_content = """
task_type: Assertion
id: check_user_age
field_path: user.age
operator: GreaterThan
expected_value: 18
description: Verify user is an adult
depends_on: []
condition: false
"""
    
    task = load_task_from_string(yaml_content, "yaml")
    print(f"Loaded task: {task.id}")
    print(f"Task type: {type(task).__name__}")
    assert isinstance(task, AssertionTask)
    
    # Example 2: Load TraceAssertionTask from JSON string
    json_content = """
{
  "task_type": "TraceAssertion",
  "id": "verify_performance",
  "operator": "LessThan",
  "expected_value": 5000.0,
  "description": "Ensure trace completes within 5 seconds",
  "depends_on": [],
  "condition": false,
  "assertion": {
    "TraceDuration": {}
  }
}
"""
    
    task = load_task_from_string(json_content, "json")
    print(f"Loaded task: {task.id}")
    assert isinstance(task, TraceAssertionTask)


def example_load_tasks_from_file():
    """Load multiple tasks from a YAML/JSON file."""
    
    # Load from YAML file
    tasks = load_tasks_from_file("./examples/eval_tasks_example.yaml")
    
    print(f"Loaded {len(tasks)} tasks from YAML file")
    
    # Iterate through tasks and check their types
    assertion_tasks = []
    llm_judge_tasks = []
    trace_assertion_tasks = []
    
    for task in tasks:
        if isinstance(task, AssertionTask):
            assertion_tasks.append(task)
        elif isinstance(task, LLMJudgeTask):
            llm_judge_tasks.append(task)
        elif isinstance(task, TraceAssertionTask):
            trace_assertion_tasks.append(task)
    
    print(f"  - {len(assertion_tasks)} AssertionTasks")
    print(f"  - {len(llm_judge_tasks)} LLMJudgeTasks")
    print(f"  - {len(trace_assertion_tasks)} TraceAssertionTasks")
    
    # Load from JSON file
    json_tasks = load_tasks_from_file("./examples/eval_tasks_example.json")
    print(f"\nLoaded {len(json_tasks)} tasks from JSON file")


def example_load_single_task_file():
    """Load a single task from a file."""
    
    # Create a temporary task file
    import tempfile
    import os
    
    with tempfile.NamedTemporaryFile(mode='w', suffix='.yaml', delete=False) as f:
        f.write("""
task_type: Assertion
id: check_email_format
field_path: user.email
operator: IsEmail
expected_value: true
description: Validate email format
depends_on: []
condition: false
""")
        temp_file = f.name
    
    try:
        # Load the task
        task = load_task_from_file(temp_file)
        print(f"Loaded single task: {task.id}")
        assert isinstance(task, AssertionTask)
        assert task.operator.name == "IsEmail"
    finally:
        os.unlink(temp_file)


def example_working_with_loaded_tasks():
    """Demonstrate working with loaded tasks."""
    
    yaml_content = """
tasks:
  - task_type: Assertion
    id: check_age
    field_path: user.age
    operator: GreaterThanOrEqual
    expected_value: 18
    description: Check minimum age
    depends_on: []
    condition: false
    
  - task_type: Assertion
    id: check_name
    field_path: user.name
    operator: HasLengthGreaterThan
    expected_value: 0
    description: Check name is not empty
    depends_on: []
    condition: false
    
  - task_type: TraceAssertion
    id: check_trace_duration
    operator: LessThan
    expected_value: 1000.0
    description: Check trace is fast
    depends_on: []
    condition: false
    assertion:
      TraceDuration: {}
"""
    
    tasks = load_tasks_from_string(yaml_content, "yaml")
    
    # Filter tasks by type
    assertion_tasks = [t for t in tasks if isinstance(t, AssertionTask)]
    trace_tasks = [t for t in tasks if isinstance(t, TraceAssertionTask)]
    
    print(f"Found {len(assertion_tasks)} assertion tasks:")
    for task in assertion_tasks:
        print(f"  - {task.id}: {task.description}")
    
    print(f"\nFound {len(trace_tasks)} trace assertion tasks:")
    for task in trace_tasks:
        print(f"  - {task.id}: {task.description}")
    
    # Access task properties
    age_task = assertion_tasks[0]
    print(f"\nAge task details:")
    print(f"  ID: {age_task.id}")
    print(f"  Field path: {age_task.field_path}")
    print(f"  Operator: {age_task.operator}")
    print(f"  Expected value: {age_task.expected_value}")
    print(f"  Depends on: {age_task.depends_on}")


def example_llm_judge_with_prompt_path():
    """Example of LLMJudgeTask with prompt loaded from file."""
    
    # Note: This requires a valid prompt file at the specified path
    yaml_content = """
task_type: LLMJudge
id: sentiment_analysis
field_path: response.text
operator: Equals
expected_value: Positive
description: Analyze sentiment using LLM
depends_on: []
max_retries: 3
condition: false
prompt:
  path: "./prompts/sentiment_judge.json"
"""
    
    # This will load the prompt from the file system
    # The prompt file should be a valid Prompt JSON/YAML file
    try:
        task = load_task_from_string(yaml_content, "yaml")
        print(f"Loaded LLMJudgeTask: {task.id}")
        print(f"Prompt model: {task.prompt.model}")
        print(f"Prompt provider: {task.prompt.provider}")
    except Exception as e:
        print(f"Note: This requires a valid prompt file: {e}")


def example_complex_trace_assertions():
    """Examples of complex trace assertion configurations."""
    
    yaml_content = """
tasks:
  # Check span sequence
  - task_type: TraceAssertion
    id: verify_workflow_order
    operator: SequenceMatches
    expected_value: true
    depends_on: []
    condition: false
    assertion:
      SpanSequence:
        span_names: ["init", "process", "finalize"]
  
  # Check span with complex filter
  - task_type: TraceAssertion
    id: verify_error_handling
    operator: Equals
    expected_value: true
    depends_on: []
    condition: false
    assertion:
      SpanExists:
        filter:
          And:
            filters:
              - WithStatus:
                  status: Error
              - WithAttribute:
                  key: error.type
  
  # Check span aggregation
  - task_type: TraceAssertion
    id: check_avg_duration
    operator: LessThan
    expected_value: 100.0
    depends_on: []
    condition: false
    assertion:
      SpanAggregation:
        filter:
          ByName:
            name: database_query
        attribute_key: duration_ms
        aggregation: Average
"""
    
    tasks = load_tasks_from_string(yaml_content, "yaml")
    
    print(f"Loaded {len(tasks)} trace assertion tasks:")
    for task in tasks:
        print(f"  - {task.id}: {task.description or 'No description'}")
        print(f"    Assertion type: {type(task.assertion).__name__}")


if __name__ == "__main__":
    print("=" * 60)
    print("Example 1: Load single task from string")
    print("=" * 60)
    example_load_single_task()
    
    print("\n" + "=" * 60)
    print("Example 2: Load single task from file")
    print("=" * 60)
    example_load_single_task_file()
    
    print("\n" + "=" * 60)
    print("Example 3: Load tasks from file")
    print("=" * 60)
    try:
        example_load_tasks_from_file()
    except Exception as e:
        print(f"Note: Example files not found: {e}")
    
    print("\n" + "=" * 60)
    print("Example 4: Working with loaded tasks")
    print("=" * 60)
    example_working_with_loaded_tasks()
    
    print("\n" + "=" * 60)
    print("Example 5: LLMJudgeTask with prompt path")
    print("=" * 60)
    example_llm_judge_with_prompt_path()
    
    print("\n" + "=" * 60)
    print("Example 6: Complex trace assertions")
    print("=" * 60)
    example_complex_trace_assertions()
