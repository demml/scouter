from scouter.evaluate import (
    AggregationType,
    ComparisonOperator,
    SpanFilter,
    SpanStatus,
    TraceAssertion,
    TraceAssertionTask,
)


def test_trace_assertion():
    filter1 = SpanFilter.by_name("llm.generate")

    assert isinstance(filter1, SpanFilter.ByName)
    assert filter1.name == "llm.generate"

    assertion = TraceAssertion.span_count(filter=filter1)
    assert isinstance(assertion, TraceAssertion.SpanCount)
    assert assertion.filter.name == "llm.generate"


def test_by_name_filter():
    """Test exact name matching filter."""
    span_filter = SpanFilter.by_name("llm.generate")

    assert isinstance(span_filter, SpanFilter.ByName)
    assert span_filter.name == "llm.generate"


def test_by_name_pattern_filter():
    """Test regex pattern matching filter."""
    span_filter = SpanFilter.by_name_pattern(r"llm\..*")

    assert isinstance(span_filter, SpanFilter.ByNamePattern)
    assert span_filter.pattern == r"llm\..*"


def test_with_attribute_filter():
    """Test filter for spans with specific attribute key."""
    span_filter = SpanFilter.with_attribute("model")

    assert isinstance(span_filter, SpanFilter.WithAttribute)
    assert span_filter.key == "model"


def test_with_status_filter():
    """Test filter for span status."""
    span_filter = SpanFilter.with_status(SpanStatus.Error)

    assert isinstance(span_filter, SpanFilter.WithStatus)
    assert span_filter.status == SpanStatus.Error


def test_with_duration_filter_min_only():
    """Test duration filter with minimum constraint."""
    span_filter = SpanFilter.with_duration(min_ms=100.0)

    assert isinstance(span_filter, SpanFilter.WithDuration)
    assert span_filter.min_ms == 100.0
    assert span_filter.max_ms is None


def test_with_duration_filter_max_only():
    """Test duration filter with maximum constraint."""
    span_filter = SpanFilter.with_duration(max_ms=5000.0)
    assert isinstance(span_filter, SpanFilter.WithDuration)
    assert span_filter.min_ms is None
    assert span_filter.max_ms == 5000.0


def test_with_duration_filter_both():
    """Test duration filter with both min and max constraints."""
    span_filter = SpanFilter.with_duration(min_ms=100.0, max_ms=5000.0)

    assert isinstance(span_filter, SpanFilter.WithDuration)
    assert span_filter.min_ms == 100.0
    assert span_filter.max_ms == 5000.0


def test_sequence_filter():
    """Test span sequence filter."""
    names = ["validate", "process", "output"]
    span_filter = SpanFilter.sequence(names)

    assert isinstance(span_filter, SpanFilter.Sequence)
    assert span_filter.names == names


def test_and_filter_combination():
    """Test combining filters with AND logic."""
    span_filter1 = SpanFilter.by_name("llm.generate")
    span_filter2 = SpanFilter.with_attribute("model")
    combined = span_filter1.and_(span_filter2)

    assert isinstance(combined, SpanFilter.And)
    assert len(combined.filters) == 2
    assert isinstance(combined.filters[0], SpanFilter.ByName)
    assert isinstance(combined.filters[1], SpanFilter.WithAttribute)


def test_and_filter_chaining():
    """Test chaining multiple AND filters."""
    span_filter1 = SpanFilter.by_name("llm.generate")
    span_filter2 = SpanFilter.with_attribute("model")
    span_filter3 = SpanFilter.with_status(SpanStatus.Ok)

    combined = span_filter1.and_(span_filter2).and_(span_filter3)

    assert isinstance(combined, SpanFilter.And)
    assert len(combined.filters) == 3


def test_or_filter_combination():
    """Test combining filters with OR logic."""
    filter1 = SpanFilter.by_name("retry")
    filter2 = SpanFilter.with_status(SpanStatus.Error)
    combined = filter1.or_(filter2)

    assert isinstance(combined, SpanFilter.Or)
    assert len(combined.filters) == 2


def test_or_filter_chaining():
    """Test chaining multiple OR filters."""
    filter1 = SpanFilter.by_name("retry")
    filter2 = SpanFilter.with_status(SpanStatus.Error)
    filter3 = SpanFilter.with_duration(min_ms=10000.0)

    combined = filter1.or_(filter2).or_(filter3)

    assert isinstance(combined, SpanFilter.Or)
    assert len(combined.filters) == 3


def test_complex_filter_combination():
    """Test complex nested AND/OR filter logic."""
    # (name="llm.*" AND has_attribute="model") OR status=Error
    llm_filter = SpanFilter.by_name_pattern("llm.*")
    model_filter = SpanFilter.with_attribute("model")
    error_filter = SpanFilter.with_status(SpanStatus.Error)

    combined = llm_filter.and_(model_filter).or_(error_filter)

    assert isinstance(combined, SpanFilter.Or)
    assert len(combined.filters) == 2
    assert isinstance(combined.filters[0], SpanFilter.And)


def test_status_variants():
    """Test all SpanStatus variants are accessible."""
    assert hasattr(SpanStatus, "Ok")
    assert hasattr(SpanStatus, "Error")
    assert hasattr(SpanStatus, "Unset")


def test_status_in_filter():
    """Test using SpanStatus in filters."""
    ok_filter = SpanFilter.with_status(SpanStatus.Ok)
    error_filter = SpanFilter.with_status(SpanStatus.Error)
    unset_filter = SpanFilter.with_status(SpanStatus.Unset)

    assert ok_filter.status == SpanStatus.Ok
    assert error_filter.status == SpanStatus.Error
    assert unset_filter.status == SpanStatus.Unset


def test_aggregation_variants():
    """Test all AggregationType variants are accessible."""
    assert hasattr(AggregationType, "Count")
    assert hasattr(AggregationType, "Sum")
    assert hasattr(AggregationType, "Average")
    assert hasattr(AggregationType, "Min")
    assert hasattr(AggregationType, "Max")
    assert hasattr(AggregationType, "First")
    assert hasattr(AggregationType, "Last")


def test_span_sequence_assertion():
    """Test span sequence assertion creation."""
    names = ["call_tool", "run_agent", "double_check"]
    assertion = TraceAssertion.span_sequence(names)

    assert isinstance(assertion, TraceAssertion.SpanSequence)
    assert assertion.span_names == names


def test_span_set_assertion():
    """Test span set assertion creation."""
    names = ["validate", "process", "output"]
    assertion = TraceAssertion.span_set(names)

    assert isinstance(assertion, TraceAssertion.SpanSet)
    assert assertion.span_names == names


def test_span_count_assertion():
    """Test span count assertion with filter."""
    filter = SpanFilter.by_name("retry_operation")
    assertion = TraceAssertion.span_count(filter)

    assert isinstance(assertion, TraceAssertion.SpanCount)
    assert isinstance(assertion.filter, SpanFilter.ByName)
    assert assertion.filter.name == "retry_operation"


def test_span_exists_assertion():
    """Test span existence assertion."""
    filter = SpanFilter.by_name_pattern("llm.*")
    assertion = TraceAssertion.span_exists(filter)

    assert isinstance(assertion, TraceAssertion.SpanExists)
    assert isinstance(assertion.filter, SpanFilter.ByNamePattern)


def test_span_attribute_assertion():
    """Test span attribute extraction assertion."""
    filter = SpanFilter.by_name("llm.generate")
    assertion = TraceAssertion.span_attribute(filter, "model")

    assert isinstance(assertion, TraceAssertion.SpanAttribute)
    assert assertion.attribute_key == "model"
    assert assertion.filter.name == "llm.generate"


def test_span_duration_assertion():
    """Test span duration assertion."""
    filter = SpanFilter.by_name("database_query")
    assertion = TraceAssertion.span_duration(filter)

    assert isinstance(assertion, TraceAssertion.SpanDuration)
    assert isinstance(assertion.filter, SpanFilter.ByName)


def test_span_aggregation_assertion():
    """Test span aggregation assertion."""
    filter = SpanFilter.by_name_pattern("llm.*")
    assertion = TraceAssertion.span_aggregation(filter, "token_count", AggregationType.Sum)

    assert isinstance(assertion, TraceAssertion.SpanAggregation)
    assert assertion.attribute_key == "token_count"
    assert assertion.aggregation == AggregationType.Sum


def test_trace_duration_assertion():
    """Test trace-level duration assertion."""
    assertion = TraceAssertion.trace_duration()

    assert isinstance(assertion, TraceAssertion.TraceDuration)


def test_trace_span_count_assertion():
    """Test trace-level span count assertion."""
    assertion = TraceAssertion.trace_span_count()

    assert isinstance(assertion, TraceAssertion.TraceSpanCount)


def test_trace_error_count_assertion():
    """Test trace-level error count assertion."""
    assertion = TraceAssertion.trace_error_count()

    assert isinstance(assertion, TraceAssertion.TraceErrorCount)


def test_trace_service_count_assertion():
    """Test trace-level service count assertion."""
    assertion = TraceAssertion.trace_service_count()

    assert isinstance(assertion, TraceAssertion.TraceServiceCount)


def test_trace_max_depth_assertion():
    """Test trace-level max depth assertion."""
    assertion = TraceAssertion.trace_max_depth()

    assert isinstance(assertion, TraceAssertion.TraceMaxDepth)


def test_trace_attribute_assertion():
    """Test trace-level attribute assertion."""
    assertion = TraceAssertion.trace_attribute("user_id")

    assert isinstance(assertion, TraceAssertion.TraceAttribute)
    assert assertion.attribute_key == "user_id"


def test_basic_task_creation():
    """Test creating a basic trace assertion task."""
    filter = SpanFilter.by_name("llm.generate")
    assertion = TraceAssertion.span_count(filter)

    task = TraceAssertionTask(
        id="count_llm_calls",
        assertion=assertion,
        expected_value=5,
        operator=ComparisonOperator.Equals,
    )

    assert task.id == "count_llm_calls"
    assert isinstance(task.assertion, TraceAssertion.SpanCount)
    assert task.operator == ComparisonOperator.Equals
    assert task.expected_value == 5


def test_task_with_description():
    """Test task creation with description."""
    assertion = TraceAssertion.trace_duration()

    task = TraceAssertionTask(
        id="check_latency",
        assertion=assertion,
        expected_value=5000.0,
        operator=ComparisonOperator.LessThan,
        description="Ensure trace completes within 5 seconds",
    )

    assert task.description == "Ensure trace completes within 5 seconds"


def test_task_with_dependencies():
    """Test task creation with dependencies."""
    assertion = TraceAssertion.trace_error_count()

    task = TraceAssertionTask(
        id="no_errors",
        assertion=assertion,
        expected_value=0,
        operator=ComparisonOperator.Equals,
        depends_on=["check_latency", "count_llm_calls"],
    )

    assert len(task.depends_on) == 2
    assert "check_latency" in task.depends_on
    assert "count_llm_calls" in task.depends_on


def test_task_with_condition_flag():
    """Test task creation with condition flag."""
    filter = SpanFilter.with_status(SpanStatus.Ok)
    assertion = TraceAssertion.span_exists(filter)

    task = TraceAssertionTask(
        id="require_success",
        assertion=assertion,
        expected_value=True,
        operator=ComparisonOperator.Equals,
        condition=True,
    )

    assert task.condition is True


def test_task_id_lowercase_conversion():
    """Test that task IDs are converted to lowercase."""
    assertion = TraceAssertion.trace_span_count()

    task = TraceAssertionTask(
        id="CheckSpanCount",
        assertion=assertion,
        expected_value=10,
        operator=ComparisonOperator.LessThanOrEqual,
    )

    assert task.id == "checkspancount"


def test_task_with_complex_expected_value():
    """Test task with dict expected value."""
    filter = SpanFilter.by_name("llm.generate")
    assertion = TraceAssertion.span_attribute(filter, "response")

    expected = {"status": "success", "tokens": 150}
    task = TraceAssertionTask(
        id="check_response",
        assertion=assertion,
        expected_value=expected,
        operator=ComparisonOperator.Contains,
    )

    # Access expected_value through getter
    assert task.expected_value == expected


def test_task_with_list_expected_value():
    """Test task with list expected value."""
    names = ["step1", "step2", "step3"]
    assertion = TraceAssertion.span_sequence(names)

    task = TraceAssertionTask(
        id="verify_sequence",
        assertion=assertion,
        expected_value=True,
        operator=ComparisonOperator.Equals,
    )

    assert task.expected_value is True


def test_task_string_representation():
    """Test task string representation."""
    assertion = TraceAssertion.trace_duration()

    task = TraceAssertionTask(
        id="latency_check",
        assertion=assertion,
        expected_value=1000.0,
        operator=ComparisonOperator.LessThan,
    )

    task_str = str(task)
    assert "latency_check" in task_str
    assert isinstance(task_str, str)


def test_agent_workflow_validation():
    """Test validating agent execution order."""
    names = ["call_tool", "run_agent", "double_check"]
    assertion = TraceAssertion.span_sequence(names)

    task = TraceAssertionTask(
        id="verify_agent_workflow",
        assertion=assertion,
        expected_value=True,
        operator=ComparisonOperator.Equals,
        description="Verify correct agent execution order",
    )

    assert isinstance(task.assertion, TraceAssertion.SpanSequence)
    assert task.assertion.span_names == names


def test_retry_limit_check():
    """Test limiting retry attempts."""
    filter = SpanFilter.by_name("retry_operation")
    assertion = TraceAssertion.span_count(filter)

    task = TraceAssertionTask(
        id="limit_retries",
        assertion=assertion,
        expected_value=3,
        operator=ComparisonOperator.LessThanOrEqual,
        description="Ensure no more than 3 retry attempts",
    )

    assert task.expected_value == 3
    assert task.operator == ComparisonOperator.LessThanOrEqual


def test_model_validation_check():
    """Test verifying correct model was used."""
    filter = SpanFilter.by_name("llm.generate")
    assertion = TraceAssertion.span_attribute(filter, "model")

    task = TraceAssertionTask(
        id="verify_model",
        assertion=assertion,
        expected_value="gpt-4",
        operator=ComparisonOperator.Equals,
        description="Verify gpt-4 was used",
    )

    assert task.assertion.attribute_key == "model"


def test_token_budget_check():
    """Test limiting total token usage."""
    filter = SpanFilter.by_name_pattern("llm.*")
    assertion = TraceAssertion.span_aggregation(filter, "token_count", AggregationType.Sum)

    task = TraceAssertionTask(
        id="token_budget",
        assertion=assertion,
        expected_value=10000,
        operator=ComparisonOperator.LessThan,
        description="Ensure total tokens under budget",
    )

    assert task.assertion.aggregation == AggregationType.Sum


def test_error_free_execution():
    """Test ensuring no errors occurred."""
    assertion = TraceAssertion.trace_error_count()

    task = TraceAssertionTask(
        id="no_errors",
        assertion=assertion,
        expected_value=0,
        operator=ComparisonOperator.Equals,
        description="Verify error-free execution",
    )

    assert task.expected_value == 0


def test_performance_sla_check():
    """Test enforcing performance SLA."""
    assertion = TraceAssertion.trace_duration()

    task = TraceAssertionTask(
        id="performance_sla",
        assertion=assertion,
        expected_value=5000.0,  # 5 seconds
        operator=ComparisonOperator.LessThan,
        description="Ensure execution completes within 5 seconds",
    )

    assert task.expected_value == 5000.0


def test_chained_assertions_with_dependencies():
    """Test multiple assertions with dependency chain."""
    # First check: verify workflow
    workflow_assertion = TraceAssertion.span_sequence(["validate", "process", "output"])
    _workflow_task = TraceAssertionTask(
        id="workflow_check",
        assertion=workflow_assertion,
        expected_value=True,
        operator=ComparisonOperator.Equals,
    )

    # Second check: verify performance (depends on workflow)
    perf_assertion = TraceAssertion.trace_duration()
    perf_task = TraceAssertionTask(
        id="performance_check",
        assertion=perf_assertion,
        expected_value=3000.0,
        operator=ComparisonOperator.LessThan,
        depends_on=["workflow_check"],
    )

    # Third check: verify no errors (depends on both)
    error_assertion = TraceAssertion.trace_error_count()
    error_task = TraceAssertionTask(
        id="error_check",
        assertion=error_assertion,
        expected_value=0,
        operator=ComparisonOperator.Equals,
        depends_on=["workflow_check", "performance_check"],
    )

    assert len(perf_task.depends_on) == 1
    assert len(error_task.depends_on) == 2
