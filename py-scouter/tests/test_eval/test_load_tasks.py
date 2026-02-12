from pathlib import Path

from scouter.evaluate import (
    AssertionTask,
    ComparisonOperator,
    EvaluationTaskType,
    LLMJudgeTask,
    SpanFilter,
    SpanStatus,
    TasksFile,
    TraceAssertion,
    TraceAssertionTask,
)


def test_load_all_tasks_from_file_yaml():
    """Verify all 13 tasks are loaded with correct types and properties."""
    file_path = Path(__file__).parent / "assets" / "eval_tasks_example.yaml"
    tasks = TasksFile.from_path(file_path)

    assert len(tasks) == 13, f"Expected 13 tasks, got {len(tasks)}"

    # Example 1: Simple assertion checking user age
    task1 = tasks[0]
    assert isinstance(task1, AssertionTask)
    assert task1.task_type == EvaluationTaskType.Assertion
    assert task1.id == "check_user_age"
    assert task1.field_path == "user.age"
    assert task1.operator == ComparisonOperator.GreaterThan
    assert task1.expected_value == 18
    assert task1.description == "Verify user is an adult"
    assert task1.depends_on == []

    # Example 2: Email format validation
    task2 = tasks[1]
    assert isinstance(task2, AssertionTask)
    assert task2.id == "validate_email"
    assert task2.field_path == "user.email"
    assert task2.operator == ComparisonOperator.IsEmail
    assert task2.expected_value is True

    # Example 3: Password strength check with dependency
    task3 = tasks[2]
    assert isinstance(task3, AssertionTask)
    assert task3.id == "check_password_strength"
    assert task3.field_path == "user.password"
    assert task3.operator == ComparisonOperator.HasLengthGreaterThanOrEqual
    assert task3.expected_value == 8
    assert task3.depends_on == ["validate_email"]

    # Example 4: Response array length check
    task4 = tasks[3]
    assert isinstance(task4, AssertionTask)
    assert task4.id == "validate_response_items"
    assert task4.field_path == "response.data.items"
    assert task4.operator == ComparisonOperator.HasLengthGreaterThan
    assert task4.expected_value == 0

    # Example 5: String contains check
    task5 = tasks[4]
    assert isinstance(task5, AssertionTask)
    assert task5.id == "check_sentiment"
    assert task5.field_path == "analysis.sentiment"
    assert task5.operator == ComparisonOperator.Contains
    assert task5.expected_value == "positive"

    # Example 6: LLM Judge task with external prompt
    task6 = tasks[5]
    assert isinstance(task6, LLMJudgeTask)
    assert task6.task_type == EvaluationTaskType.LLMJudge
    assert task6.id == "sentiment_judge"
    assert task6.field_path == "response.text"
    assert task6.operator == ComparisonOperator.Equals
    assert task6.expected_value == "Positive"
    assert task6.depends_on == ["check_sentiment"]
    assert task6.max_retries == 3
    assert task6.prompt is not None

    # Example 7: Trace assertion - span sequence
    task7 = tasks[6]
    assert isinstance(task7, TraceAssertionTask)
    assert task7.task_type == EvaluationTaskType.TraceAssertion
    assert task7.id == "verify_agent_workflow"
    assert task7.operator == ComparisonOperator.SequenceMatches
    assert task7.expected_value is True
    assert task7.description == "Verify agent workflow execution order"

    # Example 8: Trace assertion - span count
    task8 = tasks[7]
    assert isinstance(task8, TraceAssertionTask)
    assert task8.id == "verify_retry_count"
    assert task8.operator == ComparisonOperator.LessThanOrEqual
    assert task8.expected_value == 3

    # Example 9: Trace assertion - trace duration
    task9 = tasks[8]
    assert isinstance(task9, TraceAssertionTask)
    assert task9.id == "verify_performance"
    assert task9.operator == ComparisonOperator.LessThan
    assert task9.expected_value == 5000.0

    # Example 10: Trace assertion - span attribute with dependency
    task10 = tasks[9]
    assert isinstance(task10, TraceAssertionTask)
    assert task10.id == "verify_model_used"
    assert task10.operator == ComparisonOperator.Equals
    assert task10.expected_value == "gpt-4"
    assert task10.depends_on == ["verify_agent_workflow"]

    # Example 11: Complex span filter with AND logic
    task11 = tasks[10]
    assert isinstance(task11, TraceAssertionTask)
    assert task11.id == "verify_error_handling"
    assert task11.assertion == TraceAssertion.span_exists(
        SpanFilter.with_status(SpanStatus.Error).and_(
            SpanFilter.with_attribute("error.type"),
        )
    )
    assert task11.operator == ComparisonOperator.Equals
    assert task11.expected_value is True

    # Example 12: Trace assertion - span duration range
    task12 = tasks[11]
    assert isinstance(task12, TraceAssertionTask)
    assert task12.id == "verify_span_duration"
    assert task12.operator == ComparisonOperator.Equals
    assert task12.expected_value is True

    # Example 13: Conditional task with multiple dependencies
    task13 = tasks[12]
    assert isinstance(task13, AssertionTask)
    assert task13.id == "final_validation"
    assert task13.field_path == "status"
    assert task13.operator == ComparisonOperator.Equals
    assert task13.expected_value == "success"
    assert set(task13.depends_on) == {
        "check_user_age",
        "validate_email",
        "check_password_strength",
    }


def test_task_iteration():
    """Verify tasks can be iterated and indexed correctly."""
    file_path = Path(__file__).parent / "assets" / "eval_tasks_example.yaml"
    tasks = TasksFile.from_path(file_path)

    # Test __len__
    assert len(tasks) == 13

    # Test __getitem__ with index
    first_task = tasks[0]
    assert first_task.id == "check_user_age"

    last_task = tasks[12]
    assert last_task.id == "final_validation"

    # Test __getitem__ with slice
    first_three = tasks[0:3]
    assert isinstance(first_three, list)
    assert len(first_three) == 3
    assert first_three[0].id == "check_user_age"
    assert first_three[1].id == "validate_email"
    assert first_three[2].id == "check_password_strength"

    # Test iteration
    task_ids = [task.id for task in tasks]
    assert len(task_ids) == 13
    assert task_ids[0] == "check_user_age"
    assert task_ids[5] == "sentiment_judge"
    assert task_ids[12] == "final_validation"


def test_load_all_tasks_from_file_json():
    """Verify all 13 tasks are loaded with correct types and properties."""
    file_path = Path(__file__).parent / "assets" / "eval_tasks_example.json"
    tasks = TasksFile.from_path(file_path)

    assert len(tasks) == 2, f"Expected 2 tasks, got {len(tasks)}"

    # Example 1: Simple assertion checking user age
    task1 = tasks[0]
    assert isinstance(task1, AssertionTask)
    assert task1.id == "validate_email"
    assert task1.field_path == "user.email"
    assert task1.operator == ComparisonOperator.IsEmail
    assert task1.expected_value is True


def test_load_task_array_from_file():
    """Verify all 13 tasks are loaded with correct types and properties."""
    file_path = Path(__file__).parent / "assets" / "tasks_example.yaml"
    _tasks = TasksFile.from_path(file_path)
