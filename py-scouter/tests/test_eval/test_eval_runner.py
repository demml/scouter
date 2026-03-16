import time

import pytest
from scouter.drift import GenAIEvalProfile
from scouter.evaluate import (
    AssertionTask,
    ComparisonOperator,
    EvalRecord,
    EvalRunner,
    EvalScenario,
    EvalScenarios,
    SpanFilter,
    TraceAssertion,
    TraceAssertionTask,
)
from scouter.mock import MockConfig
from scouter.queue import ScouterQueue
from scouter.tracing import TestSpanExporter, init_tracer

# ---------------------------------------------------------------------------
# Scenario data: (query, quality, count, expected_pass)
# ---------------------------------------------------------------------------
SCENARIO_DATA = [
    ("Query 1", 8, 3, True),
    ("Query 2", 9, 5, True),
    ("Query 3", 7, 2, True),
    ("Query 4", 4, 0, False),
    ("Query 5", 6, 1, False),
]


def _build_profiles():
    retriever_profile = GenAIEvalProfile(
        tasks=[
            AssertionTask(
                id="result_count",
                context_path="results.count",
                operator=ComparisonOperator.GreaterThanOrEqual,
                expected_value=1,
            ),
            TraceAssertionTask(
                id="retriever_span_exists",
                assertion=TraceAssertion.span_count(SpanFilter.by_name("retriever_call")),
                operator=ComparisonOperator.GreaterThanOrEqual,
                expected_value=1,
            ),
        ],
        alias="retriever",
    )
    synthesizer_profile = GenAIEvalProfile(
        tasks=[
            AssertionTask(
                id="quality_check",
                context_path="response.quality",
                operator=ComparisonOperator.GreaterThanOrEqual,
                expected_value=7,
            ),
        ],
        alias="synthesizer",
    )
    return retriever_profile, synthesizer_profile


def _build_scenarios():
    return EvalScenarios(
        scenarios=[
            EvalScenario(
                initial_query=query,
                id=f"scenario_{i + 1}",
                expected_outcome="High quality response with sufficient results",
                tasks=[
                    AssertionTask(
                        id="response_not_empty",
                        context_path="response",
                        operator=ComparisonOperator.IsString,
                        expected_value=True,
                    ),
                ],
            )
            for i, (query, _, _, _) in enumerate(SCENARIO_DATA)
        ]
    )


def test_eval_runner_full_e2e(tracer):
    """Full E2E: 5 scenarios, 2 sub-agents, span capture, 3-level metrics."""
    retriever_profile, synthesizer_profile = _build_profiles()
    scenarios = _build_scenarios()

    runner = EvalRunner(
        scenarios=scenarios,
        profiles={"retriever": retriever_profile, "synthesizer": synthesizer_profile},
    )

    tracer.enable_local_capture()

    for i, (query, quality, count, _) in enumerate(SCENARIO_DATA):
        scenario = scenarios.scenarios[i]
        record_id = f"scenario_{i + 1}"

        with tracer.start_as_current_span("retriever_call") as span:
            trace_id_hex = format(span.get_span_context().trace_id, "032x")

        retriever_records = [
            EvalRecord(
                context={"results": {"count": count, "query": query}},
                trace_id=trace_id_hex,
                id=record_id,
            )
        ]
        synthesizer_records = [
            EvalRecord(
                context={"response": {"quality": quality, "text": f"Answer for {query}"}},
                trace_id=trace_id_hex,
                id=record_id,
            )
        ]

        runner.collect_scenario_data(
            records={"retriever": retriever_records, "synthesizer": synthesizer_records},
            response=f"Synthesized answer for: {query}",
            scenario=scenario,
        )

    # Allow batch span export to flush to capture buffer
    time.sleep(0.2)

    results = runner.evaluate()

    # ── Level 3: aggregate metrics ──
    assert results.metrics.total_scenarios == 5
    # Scenario tasks only check "response IsString" — all 5 pass at scenario level
    assert results.metrics.passed_scenarios == 5
    # Retriever: result_count >= 1 → scenarios 1,2,3,5 pass (count=3,5,2,1), scenario 4 fails (count=0) → 4/5 = 0.8
    # Synthesizer: quality >= 7 → scenarios 1,2,3 pass (8,9,7), 4,5 fail (4,6) → 3/5 = 0.6
    # Scenario tasks: response IsString → all 5 pass → 5/5 = 1.0
    # Note: trace tasks (retriever_span_exists) also run per record — spans are present for all 5
    # Overall = mean of dataset + scenario rates
    assert "retriever" in results.metrics.dataset_pass_rates
    assert "synthesizer" in results.metrics.dataset_pass_rates
    # retriever has 2 tasks per record (result_count + retriever_span_exists)
    # result_count: 4/5 pass; retriever_span_exists: 5/5 pass → 9/10 = 0.9
    assert results.metrics.dataset_pass_rates["retriever"] == pytest.approx(0.9, abs=0.05)
    # synthesizer has 1 task per record (quality_check): 3/5 = 0.6
    assert results.metrics.dataset_pass_rates["synthesizer"] == pytest.approx(0.6, abs=0.05)
    # scenario_pass_rate: all 5 pass → 1.0
    assert results.metrics.scenario_pass_rate == pytest.approx(1.0, abs=0.01)
    # overall = mean(0.9, 0.6, 1.0) ≈ 0.833
    assert results.metrics.overall_pass_rate == pytest.approx(0.833, abs=0.05)

    # ── Level 1: dataset results — 5 records per alias ──
    assert "retriever" in results.dataset_results
    assert "synthesizer" in results.dataset_results
    retriever_results = results.dataset_results["retriever"]
    synthesizer_results = results.dataset_results["synthesizer"]
    assert retriever_results.successful_count + retriever_results.failed_count == 5
    assert synthesizer_results.successful_count + synthesizer_results.failed_count == 5

    # Trace task ran — verify via per-record eval_set
    record_id_sample = "scenario_1"
    aligned = retriever_results[record_id_sample]
    task_ids = [t.task_id for t in aligned.eval_set.records]
    assert "retriever_span_exists" in task_ids

    # ── Level 2: one scenario result per scenario ──
    assert len(results.scenario_results) == 5

    tracer.disable_local_capture()
    tracer.drain_local_spans()


def test_eval_runner_no_trace_tasks():
    """Runner works end-to-end without any trace tasks or tracer fixture."""
    retriever_profile = GenAIEvalProfile(
        tasks=[
            AssertionTask(
                id="result_count",
                context_path="results.count",
                operator=ComparisonOperator.GreaterThanOrEqual,
                expected_value=1,
            ),
        ],
        alias="retriever",
    )

    scenario = EvalScenario(
        initial_query="What is 2+2?",
        id="math_scenario",
        expected_outcome="Correct arithmetic answer",
        tasks=[
            AssertionTask(
                id="response_is_string",
                context_path="response",
                operator=ComparisonOperator.IsString,
                expected_value=True,
            ),
        ],
    )
    scenarios = EvalScenarios(scenarios=[scenario])

    runner = EvalRunner(
        scenarios=scenarios,
        profiles={"retriever": retriever_profile},
    )

    records = [EvalRecord(context={"results": {"count": 2}}, id="rec_1")]
    runner.collect_scenario_data(
        records={"retriever": records},
        response="The answer is 4",
        scenario=scenario,
    )

    results = runner.evaluate()

    assert results.metrics.total_scenarios == 1
    assert results.metrics.passed_scenarios == 1
    retriever_results = results.dataset_results["retriever"]
    assert retriever_results.successful_count + retriever_results.failed_count == 1
    assert len(results.scenario_results) == 1


def test_mock_adk_agent_e2e():
    """Mock Google ADK agent: ScouterQueue capture + span.add_queue_item + EvalRunner."""
    ADK_SCENARIO_DATA = [
        ("What is RAG?", 8, 3),
        ("How does LLM work?", 7, 2),
        ("What is a vector?", 5, 1),
    ]

    # 1. Build profiles
    retriever_profile = GenAIEvalProfile(
        tasks=[
            AssertionTask(
                id="result_count",
                context_path="results.count",
                operator=ComparisonOperator.GreaterThanOrEqual,
                expected_value=1,
            ),
            TraceAssertionTask(
                id="retriever_span_exists",
                assertion=TraceAssertion.span_count(SpanFilter.by_name("agent_call")),
                operator=ComparisonOperator.GreaterThanOrEqual,
                expected_value=1,
            ),
        ],
        alias="retriever",
    )
    synthesizer_profile = GenAIEvalProfile(
        tasks=[
            AssertionTask(
                id="quality_check",
                context_path="response.quality",
                operator=ComparisonOperator.GreaterThanOrEqual,
                expected_value=7,
            ),
        ],
        alias="synthesizer",
    )

    # 2. Create ScouterQueue from both profiles + enable capture
    queue = ScouterQueue.from_profile(
        profile=[retriever_profile, synthesizer_profile],
        transport_config=MockConfig(),
        wait_for_startup=True,
    )
    queue.enable_capture()

    # 3. Wire queue into tracer via init_tracer (idempotent: provider not re-created;
    #    new BaseTracer instance returned with queue so add_queue_item works)
    adk_tracer = init_tracer(
        service_name="mock-adk-agent",
        scouter_queue=queue,
        transport_config=MockConfig(),
        exporter=TestSpanExporter(batch_export=False),
    )
    adk_tracer.enable_local_capture()

    # 4. Build scenarios + runner
    scenarios = EvalScenarios(
        scenarios=[
            EvalScenario(
                initial_query=query,
                id=f"adk_scenario_{i + 1}",
                expected_outcome="High quality response with sufficient results",
                tasks=[
                    AssertionTask(
                        id="response_not_empty",
                        context_path="response",
                        operator=ComparisonOperator.IsString,
                        expected_value=True,
                    ),
                ],
            )
            for i, (query, _, _) in enumerate(ADK_SCENARIO_DATA)
        ]
    )
    runner = EvalRunner(scenarios=scenarios, profiles=queue.genai_profiles())

    # 5. Simulate mock ADK agent — span.add_queue_item auto-stamps trace_id onto EvalRecord
    for i, (query, quality, count) in enumerate(ADK_SCENARIO_DATA):
        scenario = scenarios.scenarios[i]
        with adk_tracer.start_as_current_span("agent_call") as span:
            span.add_queue_item(
                "retriever",
                EvalRecord(
                    context={"results": {"count": count, "query": query}},
                    id=f"retriever_{i + 1}",
                ),
            )
            span.add_queue_item(
                "synthesizer",
                EvalRecord(
                    context={"response": {"quality": quality, "text": f"Answer for {query}"}},
                    id=f"synthesizer_{i + 1}",
                ),
            )
        # Drain immediately after span ends — records carry trace_id from the span
        scenario_records = queue.drain_all_records()
        runner.collect_scenario_data(
            records=scenario_records,
            response=f"Synthesized answer for: {query}",
            scenario=scenario,
        )

    # Allow batch span export to flush to capture buffer
    time.sleep(0.2)

    # 6. Evaluate
    results = runner.evaluate()

    # ── Level 3: aggregate metrics ──
    assert results.metrics.total_scenarios == 3
    # Scenario tasks only check "response IsString" — all 3 pass at scenario level
    assert results.metrics.passed_scenarios == 3
    assert 0.0 < results.metrics.overall_pass_rate <= 1.0
    assert "retriever" in results.metrics.dataset_pass_rates
    assert "synthesizer" in results.metrics.dataset_pass_rates

    # ── Level 1: 3 records per alias ──
    retriever_results = results.dataset_results["retriever"]
    synthesizer_results = results.dataset_results["synthesizer"]
    assert retriever_results.successful_count + retriever_results.failed_count == 3
    assert synthesizer_results.successful_count + synthesizer_results.failed_count == 3

    # Trace task ran on every retriever record
    for i in range(1, 4):
        aligned = retriever_results[f"retriever_{i}"]
        task_ids = [t.task_id for t in aligned.eval_set.records]
        assert "retriever_span_exists" in task_ids

    # ── Level 2: one result per scenario ──
    assert len(results.scenario_results) == 3

    # Cleanup
    adk_tracer.disable_local_capture()
    adk_tracer.drain_local_spans()
