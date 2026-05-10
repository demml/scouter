from __future__ import annotations

import asyncio
from typing import Any

import pytest
import scouter.evaluate.runner as eval_runner_module
from scouter import trace as scouter_trace
from scouter.drift import AgentEvalProfile
from scouter.evaluate import (
    AssertionTask,
    ComparisonOperator,
    EvalOrchestrator,
    EvalScenario,
    EvalScenarios,
    ScenarioEvalResults,
)
from scouter.mock import MockConfig
from scouter.queue import ScouterQueue
from scouter.tracing import ScouterInstrumentor
from scouter.tracing import TestSpanExporter as _TestSpanExporter


def _profile(alias: str = "agent") -> AgentEvalProfile:
    return AgentEvalProfile(
        tasks=[
            AssertionTask(
                id="response_is_string",
                context_path="response",
                operator=ComparisonOperator.IsString,
                expected_value=True,
            )
        ],
        alias=alias,
    )


def _two_scenarios() -> EvalScenarios:
    return EvalScenarios(
        scenarios=[
            EvalScenario(
                initial_query="Q1",
                id="scenario_alpha",
                expected_outcome="ok",
                tasks=[
                    AssertionTask(
                        id="response_is_string",
                        context_path="response",
                        operator=ComparisonOperator.IsString,
                        expected_value=True,
                    )
                ],
            ),
            EvalScenario(
                initial_query="Q2",
                id="scenario_beta",
                expected_outcome="ok",
                tasks=[
                    AssertionTask(
                        id="response_is_string",
                        context_path="response",
                        operator=ComparisonOperator.IsString,
                        expected_value=True,
                    )
                ],
            ),
        ]
    )


@pytest.fixture
def queue() -> ScouterQueue:
    return ScouterQueue.from_profile(
        profile=[_profile()],
        transport_config=MockConfig(),
        wait_for_startup=True,
    )


def _profile_uid(queue: ScouterQueue) -> str:
    return queue.agent_profiles()["agent"].config.uid


@pytest.fixture
def tracer(queue: ScouterQueue):
    ScouterInstrumentor._instance = None
    ScouterInstrumentor._provider = None
    exporter = _TestSpanExporter(batch_export=False)
    inst = ScouterInstrumentor()
    inst.instrument(transport_config=MockConfig(), exporter=exporter, scouter_queue=queue)
    yield scouter_trace.get_tracer("orch-isolation")
    inst.uninstrument()
    ScouterInstrumentor._instance = None
    ScouterInstrumentor._provider = None


def _get_scenario(results: ScenarioEvalResults, scenario_id: str) -> Any:
    return next(r for r in results.scenario_results if r.scenario_id == scenario_id)


def test_fresh_otel_context_unavailable_returns_none(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(eval_runner_module, "_OTEL_AVAILABLE", False)
    monkeypatch.setattr(eval_runner_module, "_otel_context", None)

    assert eval_runner_module._fresh_otel_context() is None


def _assert_no_trace_bleed(results: ScenarioEvalResults) -> None:
    """Assert that no trace_id appears in more than one scenario's spans."""
    alpha = _get_scenario(results, "scenario_alpha")
    beta = _get_scenario(results, "scenario_beta")

    assert alpha.traces, "scenario_alpha captured no spans"
    assert beta.traces, "scenario_beta captured no spans"

    alpha_ids = {s.trace_id for s in alpha.traces}
    beta_ids = {s.trace_id for s in beta.traces}
    assert alpha_ids.isdisjoint(beta_ids), f"trace_id bleed between scenarios: {alpha_ids & beta_ids}"


def test_scenarios_have_distinct_trace_ids(queue: ScouterQueue, tracer: Any) -> None:
    """Wrapper spans for different scenarios must not share a trace_id."""
    scenarios = _two_scenarios()

    def agent(query: str) -> str:
        with tracer.start_as_current_span("agent_call") as span:
            span.attach_eval(profile_uid=_profile_uid(queue), context={"response": query}, record_id=f"rec_{query}")
        return query

    results = EvalOrchestrator(queue=queue, scenarios=scenarios, agent_fn=agent).run()

    _assert_no_trace_bleed(results)


def test_scenarios_pass_with_correct_spans(queue: ScouterQueue, tracer: Any) -> None:
    """Both scenarios must pass and have non-empty trace data."""
    scenarios = _two_scenarios()

    def agent(query: str) -> str:
        with tracer.start_as_current_span("agent_call") as span:
            span.attach_eval(profile_uid=_profile_uid(queue), context={"response": query}, record_id=f"rec_{query}")
        return query

    results = EvalOrchestrator(queue=queue, scenarios=scenarios, agent_fn=agent).run()

    assert results.metrics.total_scenarios == 2
    assert results.metrics.passed_scenarios == 2

    for scenario_result in results.scenario_results:
        assert scenario_result.passed, f"{scenario_result.scenario_id} failed"
        assert scenario_result.traces, f"{scenario_result.scenario_id} got no spans"


def test_async_agent_fn_preserves_isolation(queue: ScouterQueue, tracer: Any) -> None:
    """asyncio-based agent must not cause spans to cross scenario boundaries."""
    scenarios = _two_scenarios()

    async def _async_work(query: str) -> str:
        with tracer.start_as_current_span("async_child_1"):
            await asyncio.sleep(0)
        with tracer.start_as_current_span("async_child_2") as span:
            span.attach_eval(profile_uid=_profile_uid(queue), context={"response": query}, record_id=f"rec_{query}")
            await asyncio.sleep(0)
        return query

    def agent(query: str) -> str:
        return asyncio.run(_async_work(query))

    results = EvalOrchestrator(queue=queue, scenarios=scenarios, agent_fn=agent).run()

    _assert_no_trace_bleed(results)

    alpha = _get_scenario(results, "scenario_alpha")
    beta = _get_scenario(results, "scenario_beta")
    alpha_span_names = {s.span_name for s in alpha.traces}
    beta_span_names = {s.span_name for s in beta.traces}
    assert "async_child_1" in alpha_span_names
    assert "async_child_1" in beta_span_names


def test_nested_sync_spans_preserve_isolation(queue: ScouterQueue, tracer: Any) -> None:
    """Deeply nested sync spans must stay within their scenario's trace."""
    scenarios = _two_scenarios()

    def agent(query: str) -> str:
        with tracer.start_as_current_span("sync_outer"):
            with tracer.start_as_current_span("sync_inner"):
                with tracer.start_as_current_span("sync_deep") as span:
                    span.attach_eval(
                        profile_uid=_profile_uid(queue),
                        context={"response": query},
                        record_id=f"rec_{query}",
                    )
        return query

    results = EvalOrchestrator(queue=queue, scenarios=scenarios, agent_fn=agent).run()

    _assert_no_trace_bleed(results)

    for scenario_result in results.scenario_results:
        span_names = {s.span_name for s in scenario_result.traces}
        assert {"sync_outer", "sync_inner", "sync_deep"}.issubset(
            span_names
        ), f"{scenario_result.scenario_id} missing expected nested spans: {span_names}"


def test_nested_async_spans_preserve_isolation(queue: ScouterQueue, tracer: Any) -> None:
    """Nested async spans with context switching must not bleed across scenarios."""
    scenarios = _two_scenarios()

    async def _async_work(query: str) -> str:
        with tracer.start_as_current_span("async_outer"):
            await asyncio.sleep(0)
            with tracer.start_as_current_span("async_inner"):
                await asyncio.sleep(0)
                with tracer.start_as_current_span("async_deep") as span:
                    span.attach_eval(
                        profile_uid=_profile_uid(queue),
                        context={"response": query},
                        record_id=f"rec_{query}",
                    )
                    await asyncio.sleep(0)
        return query

    def agent(query: str) -> str:
        return asyncio.run(_async_work(query))

    results = EvalOrchestrator(queue=queue, scenarios=scenarios, agent_fn=agent).run()

    _assert_no_trace_bleed(results)

    for scenario_result in results.scenario_results:
        span_names = {s.span_name for s in scenario_result.traces}
        assert {"async_outer", "async_inner", "async_deep"}.issubset(
            span_names
        ), f"{scenario_result.scenario_id} missing expected nested spans: {span_names}"


def test_third_party_spans_captured_per_scenario() -> None:
    """Third-party OTel spans (e.g. ADK) inside an eval scenario must land in the correct
    scenario bucket and must not bleed across scenarios.

    Requires ScouterInstrumentor so otel_trace.get_tracer() routes through ScouterTracerProvider.
    """
    from opentelemetry import trace as otel_trace
    from scouter import trace as scouter_trace
    from scouter.tracing import ScouterInstrumentor
    from scouter.tracing import TestSpanExporter as _TestSpanExporter

    ScouterInstrumentor._instance = None
    ScouterInstrumentor._provider = None

    q = ScouterQueue.from_profile(
        profile=[_profile()],
        transport_config=MockConfig(),
        wait_for_startup=True,
    )
    exporter = _TestSpanExporter(batch_export=False)
    inst = ScouterInstrumentor()
    inst.instrument(transport_config=MockConfig(), exporter=exporter, scouter_queue=q)
    tracer = scouter_trace.get_tracer("orch-isolation-tp")

    try:
        scenarios = _two_scenarios()

        def agent(query: str) -> str:
            with tracer.start_as_current_span("wrapper") as span:
                lib_tracer = otel_trace.get_tracer("third-party-lib")
                with lib_tracer.start_as_current_span("third-party-call"):
                    pass
                span.attach_eval(profile_uid=_profile_uid(q), context={"response": query}, record_id=f"rec_{query}")
            return query

        results = EvalOrchestrator(queue=q, scenarios=scenarios, agent_fn=agent).run()

        _assert_no_trace_bleed(results)

        for scenario_result in results.scenario_results:
            span_names = {s.span_name for s in scenario_result.traces}
            assert (
                "third-party-call" in span_names
            ), f"{scenario_result.scenario_id} missing third-party-call span; got: {span_names}"
    finally:
        inst.uninstrument()
        ScouterInstrumentor._instance = None
        ScouterInstrumentor._provider = None


def test_third_party_async_spans_captured_per_scenario() -> None:
    """Third-party async spans with a context switch mid-span must still land in the correct
    scenario bucket. Validates that baggage propagates across asyncio yield points.

    Requires ScouterInstrumentor so otel_trace.get_tracer() routes through ScouterTracerProvider.
    """
    from opentelemetry import trace as otel_trace
    from scouter import trace as scouter_trace
    from scouter.tracing import ScouterInstrumentor
    from scouter.tracing import TestSpanExporter as _TestSpanExporter

    ScouterInstrumentor._instance = None
    ScouterInstrumentor._provider = None

    q = ScouterQueue.from_profile(
        profile=[_profile()],
        transport_config=MockConfig(),
        wait_for_startup=True,
    )
    exporter = _TestSpanExporter(batch_export=False)
    inst = ScouterInstrumentor()
    inst.instrument(transport_config=MockConfig(), exporter=exporter, scouter_queue=q)
    tracer = scouter_trace.get_tracer("orch-isolation-async-tp")

    try:
        scenarios = _two_scenarios()

        async def _async_agent(query: str) -> str:
            lib_tracer = otel_trace.get_tracer("third-party-async-lib")
            await asyncio.sleep(0)
            with lib_tracer.start_as_current_span("third-party-async-call"):
                await asyncio.sleep(0)
            return query

        def agent(query: str) -> str:
            with tracer.start_as_current_span("wrapper") as span:
                result = asyncio.run(_async_agent(query))
                span.attach_eval(profile_uid=_profile_uid(q), context={"response": result}, record_id=f"rec_{query}")
            return result

        results = EvalOrchestrator(queue=q, scenarios=scenarios, agent_fn=agent).run()

        _assert_no_trace_bleed(results)

        for scenario_result in results.scenario_results:
            span_names = {s.span_name for s in scenario_result.traces}
            assert (
                "third-party-async-call" in span_names
            ), f"{scenario_result.scenario_id} missing third-party-async-call span; got: {span_names}"
    finally:
        inst.uninstrument()
        ScouterInstrumentor._instance = None
        ScouterInstrumentor._provider = None


def test_all_captured_spans_carry_run_id(queue: ScouterQueue, tracer: Any) -> None:
    """Every TraceSpanRecord in every scenario result must carry scouter.eval.run_id.

    Verifies that no span was silently dropped due to missing run_id attribute.
    """
    scenarios = _two_scenarios()

    def agent(query: str) -> str:
        with tracer.start_as_current_span("outer"):
            with tracer.start_as_current_span("inner") as span:
                span.attach_eval(profile_uid=_profile_uid(queue), context={"response": query}, record_id=f"rec_{query}")
        return query

    orch = EvalOrchestrator(queue=queue, scenarios=scenarios, agent_fn=agent)
    results = orch.run()

    for scenario_result in results.scenario_results:
        assert scenario_result.traces, f"{scenario_result.scenario_id} has no captured spans"
        for span in scenario_result.traces:
            attr_keys = {a.key for a in span.attributes}
            assert "scouter.eval.run_id" in attr_keys, (
                f"span '{span.span_name}' in {scenario_result.scenario_id} missing "
                f"scouter.eval.run_id; attributes: {attr_keys}"
            )
