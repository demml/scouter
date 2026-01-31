from scouter.evaluate import (
    AggregationType,
    ComparisonOperator,
    SpanFilter,
    SpanStatus,
    TraceAssertion,
    TraceAssertionTask,
    execute_trace_assertion_tasks,
)
from scouter.mock import (
    create_sequence_pattern_trace,
    create_simple_trace,
    create_trace_with_attributes,
)


def test_trace_evaluation():
    spans = create_simple_trace()
    results = execute_trace_assertion_tasks(
        tasks=[
            TraceAssertionTask(
                id="check_id",
                assertion=TraceAssertion.span_exists(
                    filter=SpanFilter.by_name("child_1"),
                ),
                operator=ComparisonOperator.Equals,
                expected_value=True,
            ),
            TraceAssertionTask(
                id="check_id_pattern_exists",
                assertion=TraceAssertion.span_exists(
                    filter=SpanFilter.by_name_pattern("^child_.*"),
                ),
                operator=ComparisonOperator.Equals,
                expected_value=True,
            ),
            TraceAssertionTask(
                id="check_id_pattern_count",
                assertion=TraceAssertion.span_count(
                    filter=SpanFilter.by_name_pattern("^child_.*"),
                ),
                operator=ComparisonOperator.Equals,
                expected_value=2,
            ),
        ],
        spans=spans,
    )
    assert results["check_id"].passed
    assert results["check_id_pattern_exists"].passed
    assert results["check_id_pattern_count"].passed


def test_trace_attributes():
    spans = create_trace_with_attributes()

    results = execute_trace_assertion_tasks(
        tasks=[
            # Check a span's attribute equals a string
            TraceAssertionTask(
                id="check_api_call_model",
                # check a given span has an attribute
                assertion=TraceAssertion.span_attribute(
                    filter=SpanFilter.by_name("api_call"),
                    attribute_key="model",
                ),
                operator=ComparisonOperator.Equals,
                expected_value="gpt-4",
            ),
            # check a span's attribute is a dict
            TraceAssertionTask(
                id="api_call_response",
                assertion=TraceAssertion.span_attribute(
                    filter=SpanFilter.by_name("api_call"),
                    attribute_key="response",
                ),
                operator=ComparisonOperator.Equals,
                expected_value={"success": True, "data": {"id": 12345}},
            ),
            # Check count of spans with http.method = POST
            TraceAssertionTask(
                id="check_post_method_count",
                assertion=TraceAssertion.span_count(
                    filter=SpanFilter.with_attribute_value(
                        key="http.method",
                        value="POST",
                    ),
                ),
                operator=ComparisonOperator.Equals,
                expected_value=1,
            ),
            # Check spans with status OK
            TraceAssertionTask(
                id="check_status",
                assertion=TraceAssertion.span_count(
                    filter=SpanFilter.with_status(SpanStatus.Ok),
                ),
                operator=ComparisonOperator.Equals,
                expected_value=2,
            ),
            # Check spans with duration between 80ms and 120ms
            TraceAssertionTask(
                id="check_duration",
                assertion=TraceAssertion.span_count(
                    filter=SpanFilter.with_duration(min_ms=80, max_ms=120),
                ),
                operator=ComparisonOperator.Equals,
                expected_value=1,
            ),
            # CHeck Span And filter
            TraceAssertionTask(
                id="check_span_and_filter",
                assertion=TraceAssertion.span_count(
                    filter=SpanFilter.with_attribute(key="http.method").and_(
                        SpanFilter.with_status(SpanStatus.Ok)
                    ),
                ),
                operator=ComparisonOperator.Equals,
                expected_value=1,
            ),
            # Check Span Or filter
            TraceAssertionTask(
                id="check_span_or_filter",
                assertion=TraceAssertion.span_count(
                    filter=SpanFilter.with_attribute_value(
                        key="http.method", value="GET"
                    ).or_(
                        SpanFilter.with_attribute_value(key="model", value="gpt-4"),
                    ),
                ),
                operator=ComparisonOperator.Equals,
                expected_value=1,
            ),
        ],
        spans=spans,
    )

    assert results["check_api_call_model"].passed
    assert results["api_call_response"].passed
    assert results["check_post_method_count"].passed
    assert results["check_status"].passed
    assert results["check_duration"].passed
    assert results["check_span_and_filter"].passed
    assert results["check_span_or_filter"].passed


def test_trace_aggregation():
    spans = create_trace_with_attributes()

    results = execute_trace_assertion_tasks(
        tasks=[
            TraceAssertionTask(
                id="check_api_call_model",
                assertion=TraceAssertion.span_aggregation(
                    filter=SpanFilter.by_name("api_call"),
                    attribute_key="tokens.output",
                    aggregation=AggregationType.Sum,
                ),
                operator=ComparisonOperator.Equals,
                expected_value=300,
            ),
        ],
        spans=spans,
    )

    assert results["check_api_call_model"].passed


def test_trace_sequence_pattern():
    spans = create_sequence_pattern_trace()

    results = execute_trace_assertion_tasks(
        tasks=[
            TraceAssertionTask(
                id="check_sequence_pattern_exists",
                assertion=TraceAssertion.SpanCount(
                    filter=SpanFilter.sequence(names=["call_tool", "run_agent"])
                ),
                operator=ComparisonOperator.Equals,
                expected_value=2,
            ),
            TraceAssertionTask(
                id="check_call_tool",
                assertion=TraceAssertion.SpanCount(
                    filter=SpanFilter.by_name("call_tool")
                ),
                operator=ComparisonOperator.Equals,
                expected_value=2,
            ),
        ],
        spans=spans,
    )

    assert results["check_sequence_pattern_exists"].passed
    assert results["check_call_tool"].passed


def test_trace_duration():
    spans = create_trace_with_attributes()

    results = execute_trace_assertion_tasks(
        tasks=[
            TraceAssertionTask(
                id="check_duration_less_than",
                assertion=TraceAssertion.trace_duration(),
                operator=ComparisonOperator.LessThanOrEqual,
                expected_value=200,
            ),
            TraceAssertionTask(
                id="check_max_depth",
                assertion=TraceAssertion.trace_max_depth(),
                operator=ComparisonOperator.LessThanOrEqual,
                expected_value=5,
            ),
            TraceAssertionTask(
                id="check_span_count",
                assertion=TraceAssertion.trace_span_count(),
                operator=ComparisonOperator.LessThanOrEqual,
                expected_value=2,
            ),
            TraceAssertionTask(
                id="check_attribute",
                assertion=TraceAssertion.trace_attribute(attribute_key="http.method"),
                operator=ComparisonOperator.Equals,
                expected_value="POST",
            ),
        ],
        spans=spans,
    )

    assert results["check_duration_less_than"].passed
    assert results["check_duration_less_than"].actual <= 200
    assert results["check_max_depth"].passed
    assert results["check_max_depth"].actual <= 5
    assert results["check_span_count"].passed
    assert results["check_span_count"].actual <= 2
    assert results["check_attribute"].passed
