import json
import unittest.mock

import pytest
from scouter.drift import GenAIEvalProfile
from scouter.evaluate import (
    AgentAssertion,
    AgentAssertionTask,
    AssertionTask,
    AttributeFilterTask,
    ComparisonOperator,
    EvalOrchestrator,
    EvalRecord,
    EvalScenario,
    EvalScenarios,
    MultiResponseMode,
    ScenarioEvalResults,
    SpanFilter,
    TraceAssertion,
    TraceAssertionTask,
)
from scouter.mock import MockConfig
from scouter.queue import ScouterQueue
from scouter.tracing import ScouterInstrumentor, TestSpanExporter, init_tracer

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _simple_profile(alias="agent"):
    return GenAIEvalProfile(
        tasks=[
            AssertionTask(
                id="quality_check",
                context_path="response.quality",
                operator=ComparisonOperator.GreaterThanOrEqual,
                expected_value=7,
            ),
        ],
        alias=alias,
    )


def _simple_scenarios(queries):
    return EvalScenarios(
        scenarios=[
            EvalScenario(
                initial_query=q,
                id=f"scenario_{i + 1}",
                expected_outcome="good response",
                tasks=[
                    AssertionTask(
                        id="response_is_string",
                        context_path="response",
                        operator=ComparisonOperator.IsString,
                        expected_value=True,
                    ),
                ],
            )
            for i, q in enumerate(queries)
        ]
    )


def _make_queue(profiles):
    return ScouterQueue.from_profile(
        profile=profiles if isinstance(profiles, list) else [profiles],
        transport_config=MockConfig(),
        wait_for_startup=True,
    )


def _single_scenario():
    return _simple_scenarios(["What is 2+2?"])


def _three_scenarios():
    return _simple_scenarios(["Q1", "Q2", "Q3"])


# ---------------------------------------------------------------------------
# Unit Tests — default execution
# ---------------------------------------------------------------------------


def test_single_turn():
    """Default execute_agent calls agent_fn with initial_query."""
    queue = _make_queue(_simple_profile())
    scenarios = _simple_scenarios(["What is 2+2?"])
    tracer = init_tracer(
        service_name="orch-default",
        scouter_queue=queue,
        transport_config=MockConfig(),
        exporter=TestSpanExporter(batch_export=False),
    )
    call_log = []

    def my_agent(query):
        call_log.append(query)
        with tracer.start_as_current_span("agent_call") as span:
            span.add_queue_item(
                "agent",
                EvalRecord(context={"response": {"quality": 9, "text": "4"}}, id="rec_1"),
            )
        return "4"

    results = EvalOrchestrator(queue=queue, scenarios=scenarios, agent_fn=my_agent).run()

    assert call_log == ["What is 2+2?"]
    assert isinstance(results, ScenarioEvalResults)
    assert results.metrics.total_scenarios == 1
    assert results.metrics.passed_scenarios == 1


def test_multi_turn():
    """Default execute_agent calls agent_fn for initial_query + each predefined_turn."""
    queue = _make_queue(_simple_profile())
    scenarios = EvalScenarios(
        scenarios=[
            EvalScenario(
                initial_query="Plan dinner",
                predefined_turns=["Make it vegetarian"],
                id="multi_1",
                expected_outcome="dinner plan",
                tasks=[
                    AssertionTask(
                        id="response_is_string",
                        context_path="response",
                        operator=ComparisonOperator.IsString,
                        expected_value=True,
                    ),
                ],
            )
        ]
    )
    tracer = init_tracer(
        service_name="orch-multi",
        scouter_queue=queue,
        transport_config=MockConfig(),
        exporter=TestSpanExporter(batch_export=False),
    )
    call_log = []

    def my_agent(query):
        call_log.append(query)
        with tracer.start_as_current_span("agent_call") as span:
            span.add_queue_item(
                "agent",
                EvalRecord(
                    context={"response": {"quality": 8, "text": query}},
                    id=f"rec_{len(call_log)}",
                ),
            )
        return f"Response to: {query}"

    results = EvalOrchestrator(queue=queue, scenarios=scenarios, agent_fn=my_agent).run()

    assert call_log == ["Plan dinner", "Make it vegetarian"]
    assert results.metrics.total_scenarios == 1

    # Verify execute_agent returns the last turn's response, not the initial query's
    turn_log: list = []  # type: ignore

    def turn_counting_agent(query):
        turn_log.append(query)
        return f"turn_{len(turn_log)}_response"

    response = EvalOrchestrator(queue=queue, scenarios=scenarios, agent_fn=turn_counting_agent).execute_agent(
        scenarios.scenarios[0]
    )
    assert response == "turn_2_response"


# ---------------------------------------------------------------------------
# Unit Tests — subclass
# ---------------------------------------------------------------------------


def test_subclass_override():
    """Subclass overrides execute_agent — no agent_fn needed."""
    queue = _make_queue(_simple_profile())
    scenarios = _simple_scenarios(["What is 2+2?"])
    tracer = init_tracer(
        service_name="orch-subclass",
        scouter_queue=queue,
        transport_config=MockConfig(),
        exporter=TestSpanExporter(batch_export=False),
    )

    class MyOrchestrator(EvalOrchestrator):
        def execute_agent(self, scenario):
            with tracer.start_as_current_span("agent_call") as span:
                span.add_queue_item(
                    "agent",
                    EvalRecord(context={"response": {"quality": 9, "text": "4"}}, id="rec_1"),
                )
            return "4"

    results = MyOrchestrator(queue=queue, scenarios=scenarios).run()

    assert isinstance(results, ScenarioEvalResults)
    assert results.metrics.total_scenarios == 1
    assert results.metrics.passed_scenarios == 1


def test_no_agent_fn_no_override_raises():
    """NotImplementedError when neither agent_fn nor override is provided."""
    queue = _make_queue(_simple_profile())
    orch = EvalOrchestrator(queue=queue, scenarios=_simple_scenarios(["Q1"]))
    with pytest.raises(NotImplementedError, match="agent_fn"):
        orch.run()


# ---------------------------------------------------------------------------
# Unit Tests — reactive scenario
# ---------------------------------------------------------------------------


def test_reactive_raises():
    """Reactive scenario (simulated_user_persona set) raises NotImplementedError."""
    queue = _make_queue(_simple_profile())
    scenarios = EvalScenarios(
        scenarios=[
            EvalScenario(
                initial_query="Hello",
                simulated_user_persona="Curious student",
                id="reactive_1",
                expected_outcome="conversation",
                tasks=[
                    AssertionTask(
                        id="response_is_string",
                        context_path="response",
                        operator=ComparisonOperator.IsString,
                        expected_value=True,
                    ),
                ],
            )
        ]
    )
    orch = EvalOrchestrator(queue=queue, scenarios=scenarios, agent_fn=lambda q: "response")
    with pytest.raises(NotImplementedError, match="Reactive"):
        orch.run()


# ---------------------------------------------------------------------------
# Unit Tests — hook ordering
# ---------------------------------------------------------------------------


def test_hook_order():
    """Verify: on_scenario_start -> execute -> on_scenario_complete -> on_evaluation_complete."""
    queue = _make_queue(_simple_profile())
    scenarios = _simple_scenarios(["Q1"])
    tracer = init_tracer(
        service_name="orch-hooks",
        scouter_queue=queue,
        transport_config=MockConfig(),
        exporter=TestSpanExporter(batch_export=False),
    )
    hook_log = []

    class HookOrchestrator(EvalOrchestrator):
        def on_scenario_start(self, scenario):
            hook_log.append(("on_scenario_start", scenario.id))

        def execute_agent(self, scenario):
            hook_log.append(("execute_agent", scenario.id))
            with tracer.start_as_current_span("agent_call") as span:
                span.add_queue_item(
                    "agent",
                    EvalRecord(
                        context={"response": {"quality": 8, "text": "answer"}},
                        id="rec_1",
                    ),
                )
            return "answer"

        def on_scenario_complete(self, scenario, response):
            hook_log.append(("on_scenario_complete", scenario.id, response))

        def on_evaluation_complete(self, results):
            hook_log.append(("on_evaluation_complete",))
            return results

    HookOrchestrator(queue=queue, scenarios=scenarios).run()

    assert hook_log[0] == ("on_scenario_start", "scenario_1")
    assert hook_log[1] == ("execute_agent", "scenario_1")
    assert hook_log[2] == ("on_scenario_complete", "scenario_1", "answer")
    assert hook_log[3] == ("on_evaluation_complete",)


# ---------------------------------------------------------------------------
# Unit Tests — capture lifecycle
# ---------------------------------------------------------------------------


def test_capture_cleanup_on_exception():
    """enable_capture/disable_capture called even when agent_fn raises."""
    queue = _make_queue(_simple_profile())
    orch = EvalOrchestrator(
        queue=queue,
        scenarios=_simple_scenarios(["Q1"]),
        agent_fn=lambda q: (_ for _ in ()).throw(RuntimeError("agent failed")),
    )
    with pytest.raises(RuntimeError, match="agent failed"):
        orch.run()
    queue.disable_capture()


# ---------------------------------------------------------------------------
# Unit Tests — edge paths
# ---------------------------------------------------------------------------


def test_no_tracer_fallback_single_execution():
    """_execute_with_baggage falls back cleanly when no tracer is available."""
    queue = _make_queue(_simple_profile())
    call_count = 0

    def counting_agent(query):
        nonlocal call_count
        call_count += 1
        return "response"

    orch = EvalOrchestrator(queue, _single_scenario(), agent_fn=counting_agent)
    orch._execute_with_attributes(orch._scenarios.scenarios[0])
    assert call_count == 1


def test_exception_inside_span_propagates():
    """execute_agent raising inside the span context must propagate, not be swallowed."""
    queue = _make_queue(_simple_profile())
    init_tracer(
        service_name="edge-span-test",
        scouter_queue=queue,
        transport_config=MockConfig(),
        exporter=TestSpanExporter(batch_export=False),
    )
    call_count = 0

    def failing_agent(query):
        nonlocal call_count
        call_count += 1
        raise ValueError("agent failure")

    orch = EvalOrchestrator(queue, _single_scenario(), agent_fn=failing_agent)
    with pytest.raises(ValueError, match="agent failure"):
        orch._execute_with_attributes(orch._scenarios.scenarios[0])
    assert call_count == 1


def test_teardown_runs_on_exception():
    """_teardown_capture must run even when execute_agent raises mid-loop."""
    queue = _make_queue(_simple_profile())
    teardown_called = False
    original_teardown = EvalOrchestrator._teardown_capture

    def patched_teardown(self):
        nonlocal teardown_called
        teardown_called = True
        original_teardown(self)

    orch = EvalOrchestrator(
        queue,
        _single_scenario(),
        agent_fn=lambda q: (_ for _ in ()).throw(RuntimeError("boom")),
    )
    with pytest.raises(RuntimeError):
        with unittest.mock.patch.object(EvalOrchestrator, "_teardown_capture", patched_teardown):
            orch.run()
    assert teardown_called


def test_mid_loop_failure_propagates():
    """Failure on scenario N does not silently skip to scenario N+1."""
    queue = _make_queue(_simple_profile())
    executed: list = []  # type: ignore

    def agent(query):
        executed.append(query)
        if len(executed) == 2:
            raise ValueError("scenario 2 failed")
        return "ok"

    orch = EvalOrchestrator(queue, _three_scenarios(), agent_fn=agent)
    with pytest.raises(ValueError, match="scenario 2 failed"):
        orch.run()
    assert len(executed) == 2


def test_flush_tracer_failure_returns_results(monkeypatch):
    """flush_tracer() raising must not abort evaluation before results are returned."""
    import scouter.evaluate.runner as runner_mod

    queue = _make_queue(_simple_profile())
    monkeypatch.setattr(
        runner_mod,
        "flush_tracer",
        lambda: (_ for _ in ()).throw(RuntimeError("flush failed")),
    )
    results = EvalOrchestrator(queue, _single_scenario(), agent_fn=lambda q: "response").run()
    assert results is not None


def test_on_evaluation_complete_return_value_used():
    """run() must return the value from on_evaluation_complete, not the raw results."""
    queue = _make_queue(_simple_profile())
    sentinel = object()

    class CustomOrch(EvalOrchestrator):
        def execute_agent(self, scenario):
            return "response"

        def on_evaluation_complete(self, results):  # type: ignore[override]
            return sentinel

    assert CustomOrch(queue, _single_scenario()).run() is sentinel


# ---------------------------------------------------------------------------
# ADK-style integration tests
# ---------------------------------------------------------------------------

_RETRIEVER_PROFILE = GenAIEvalProfile(
    tasks=[
        AssertionTask(
            id="has_results",
            context_path="results.count",
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=1,
        ),
        TraceAssertionTask(
            id="retriever_span",
            assertion=TraceAssertion.span_count(SpanFilter.by_name("retriever_callback")),
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=1,
        ),
    ],
    alias="retriever",
)

_SYNTHESIZER_PROFILE = GenAIEvalProfile(
    tasks=[
        AssertionTask(
            id="quality_score",
            context_path="response.quality",
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=7,
        ),
        TraceAssertionTask(
            id="synthesizer_span",
            assertion=TraceAssertion.span_count(SpanFilter.by_name("synthesizer_callback")),
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=1,
        ),
    ],
    alias="synthesizer",
)

_ADK_SCENARIOS = EvalScenarios(
    scenarios=[
        EvalScenario(
            initial_query="What is retrieval augmented generation?",
            id="rag_basics",
            expected_outcome="Clear explanation of RAG with sources",
            tasks=[
                AssertionTask(
                    id="response_not_empty",
                    context_path="response",
                    operator=ComparisonOperator.IsString,
                    expected_value=True,
                ),
            ],
        ),
        EvalScenario(
            initial_query="How do transformer attention heads work?",
            id="attention_heads",
            expected_outcome="Technical explanation of multi-head attention",
            tasks=[
                AssertionTask(
                    id="response_not_empty",
                    context_path="response",
                    operator=ComparisonOperator.IsString,
                    expected_value=True,
                ),
            ],
        ),
        EvalScenario(
            initial_query="Compare BERT and GPT architectures",
            id="bert_vs_gpt",
            expected_outcome="Comparison of encoder vs decoder architectures",
            tasks=[
                AssertionTask(
                    id="response_not_empty",
                    context_path="response",
                    operator=ComparisonOperator.IsString,
                    expected_value=True,
                ),
            ],
        ),
    ]
)

_RETRIEVER_DATA = {
    "What is retrieval augmented generation?": {"count": 5, "source": "arxiv"},
    "How do transformer attention heads work?": {"count": 3, "source": "papers"},
    "Compare BERT and GPT architectures": {"count": 4, "source": "textbook"},
}

_SYNTHESIZER_DATA = {
    "What is retrieval augmented generation?": {
        "quality": 9,
        "text": "RAG combines retrieval with generation...",
    },
    "How do transformer attention heads work?": {
        "quality": 8,
        "text": "Attention heads compute scaled dot-product...",
    },
    "Compare BERT and GPT architectures": {
        "quality": 7,
        "text": "BERT uses encoder, GPT uses decoder...",
    },
}


@pytest.fixture
def adk_ctx():
    queue = ScouterQueue.from_profile(
        profile=[_RETRIEVER_PROFILE, _SYNTHESIZER_PROFILE],
        transport_config=MockConfig(),
        wait_for_startup=True,
    )
    instrumentor = ScouterInstrumentor()
    instrumentor.instrument(scouter_queue=queue, exporter=TestSpanExporter(batch_export=False))
    tracer = init_tracer(
        service_name="adk-agent",
        scouter_queue=queue,
        transport_config=MockConfig(),
        exporter=TestSpanExporter(batch_export=False),
    )
    yield queue, tracer
    instrumentor.uninstrument()


def _retriever_callback(tracer, query):
    with tracer.start_as_current_span("retriever_callback") as span:
        data = _RETRIEVER_DATA[query]
        span.add_queue_item(
            "retriever",
            EvalRecord(
                context={"results": {"count": data["count"], "source": data["source"]}},
                id=f"retriever_{query[:10]}",
            ),
        )
    return data


def _synthesizer_callback(tracer, query, data_override=None):
    with tracer.start_as_current_span("synthesizer_callback") as span:
        data = data_override or _SYNTHESIZER_DATA[query]
        span.add_queue_item(
            "synthesizer",
            EvalRecord(
                context={"response": {"quality": data["quality"], "text": data["text"]}},
                id=f"synthesizer_{query[:10]}",
            ),
        )
    return data


def _make_adk_agent_fn(tracer, synth_overrides=None):
    def agent_fn(query):
        with tracer.start_as_current_span("orchestrator_call"):
            ret_data = _retriever_callback(tracer, query)
            override = synth_overrides.get(query) if synth_overrides else None
            syn_data = _synthesizer_callback(tracer, query, data_override=override)
        return f"[{ret_data['count']} sources] {syn_data['text']}"

    return agent_fn


def _run_adk_eval(queue, tracer, synth_overrides=None):
    return EvalOrchestrator(
        queue=queue,
        scenarios=_ADK_SCENARIOS,
        agent_fn=_make_adk_agent_fn(tracer, synth_overrides),
    ).run()


def test_adk_baseline_eval(adk_ctx):
    """Run baseline eval: 3 scenarios, 2 sub-agents, all pass."""
    queue, tracer = adk_ctx
    results = _run_adk_eval(queue, tracer)

    assert results.metrics.total_scenarios == 3
    assert results.metrics.passed_scenarios == 3
    assert "retriever" in results.metrics.dataset_pass_rates
    assert "synthesizer" in results.metrics.dataset_pass_rates
    assert (
        results.dataset_results["retriever"].successful_count + results.dataset_results["retriever"].failed_count == 3
    )
    assert (
        results.dataset_results["synthesizer"].successful_count + results.dataset_results["synthesizer"].failed_count
        == 3
    )


def test_adk_save_load_roundtrip(adk_ctx, tmp_path):
    """save() → load() produces identical results."""
    queue, tracer = adk_ctx
    results = _run_adk_eval(queue, tracer)
    path = str(tmp_path / "baseline.json")
    results.save(path)
    loaded = ScenarioEvalResults.load(path)

    assert loaded.metrics.total_scenarios == results.metrics.total_scenarios
    assert loaded.metrics.passed_scenarios == results.metrics.passed_scenarios
    assert loaded.metrics.overall_pass_rate == pytest.approx(results.metrics.overall_pass_rate)
    assert len(loaded.scenario_results) == len(results.scenario_results)


def test_adk_compare_baseline_to_improved(adk_ctx, tmp_path):
    """Run baseline, run improved, compare — no regression."""
    from scouter.evaluate import ScenarioComparisonResults

    queue, tracer = adk_ctx
    baseline = _run_adk_eval(queue, tracer)
    baseline.save(str(tmp_path / "baseline.json"))

    improved = _run_adk_eval(
        queue,
        tracer,
        synth_overrides={
            "What is retrieval augmented generation?": {
                "quality": 10,
                "text": "RAG improved...",
            },
            "How do transformer attention heads work?": {
                "quality": 9,
                "text": "Attention improved...",
            },
            "Compare BERT and GPT architectures": {
                "quality": 8,
                "text": "Comparison improved...",
            },
        },
    )
    improved.save(str(tmp_path / "improved.json"))

    comp = improved.compare_to(baseline)
    assert not comp.regressed
    assert comp.comparison_overall_pass_rate >= comp.baseline_overall_pass_rate

    comp.save(str(tmp_path / "comparison.json"))
    loaded_comp = ScenarioComparisonResults.load(str(tmp_path / "comparison.json"))
    assert loaded_comp.regressed == comp.regressed


def test_adk_compare_baseline_to_regressed(adk_ctx):
    """Run baseline, run regressed, compare — regression detected."""
    queue, tracer = adk_ctx
    baseline = _run_adk_eval(queue, tracer)
    regressed = _run_adk_eval(
        queue,
        tracer,
        synth_overrides={
            "What is retrieval augmented generation?": {"quality": 3, "text": "bad"},
            "How do transformer attention heads work?": {"quality": 2, "text": "worse"},
            "Compare BERT and GPT architectures": {"quality": 4, "text": "terrible"},
        },
    )

    comp = regressed.compare_to(baseline)
    assert comp.regressed
    assert comp.comparison_overall_pass_rate < comp.baseline_overall_pass_rate


def test_multi_agent_trace_assertions():
    """AttributeFilter: run nested assertions on span attributes across Gemini responses."""

    GEMINI_FUNC_CALL = {
        "candidates": [
            {
                "content": {
                    "role": "model",
                    "parts": [
                        {
                            "functionCall": {
                                "name": "transfer_to_agent",
                                "args": {"agent_name": "MeatRecipeAgent"},
                            }
                        }
                    ],
                },
                "finishReason": "STOP",
            }
        ],
        "usageMetadata": {"promptTokenCount": 800, "candidatesTokenCount": 802},
    }
    GEMINI_TEXT = {
        "candidates": [
            {
                "content": {
                    "role": "model",
                    "parts": [{"text": "Pan-Seared Ribeye Steak..."}],
                },
                "finishReason": "STOP",
            }
        ],
        "usageMetadata": {"promptTokenCount": 1200, "candidatesTokenCount": 1089},
    }

    profile = GenAIEvalProfile(
        tasks=[
            AssertionTask(
                id="placeholder",
                operator=ComparisonOperator.IsString,
                expected_value=True,
                context_path="query",
            ),
        ],
        alias="agent",
    )
    queue = ScouterQueue.from_profile(
        profile=[profile],
        transport_config=MockConfig(),
        wait_for_startup=True,
    )

    scenarios = EvalScenarios(
        scenarios=[
            EvalScenario(
                initial_query="Give me a meat recipe",
                id="adk_multi",
                expected_outcome="Recipe",
                tasks=[
                    TraceAssertionTask(
                        id="transfer_called",
                        assertion=TraceAssertion.attribute_filter(
                            key="gen_ai.response",
                            task=AttributeFilterTask.assertion(
                                AssertionTask(
                                    id="agent_name_check",
                                    context_path="candidates",
                                    operator=ComparisonOperator.HasLengthGreaterThan,
                                    expected_value=0,
                                )
                            ),
                            mode=MultiResponseMode.Any,
                        ),
                        operator=ComparisonOperator.Equals,
                        expected_value=True,
                    ),
                ],
            )
        ]
    )

    instrumentor = ScouterInstrumentor()
    instrumentor.instrument(scouter_queue=queue, exporter=TestSpanExporter(batch_export=False))
    tracer = init_tracer(
        service_name="adk",
        scouter_queue=queue,
        transport_config=MockConfig(),
        exporter=TestSpanExporter(batch_export=False),
    )

    def mock_adk(query):
        with tracer.start_as_current_span("router.generate") as span:
            span.set_attribute("gen_ai.response", json.dumps(GEMINI_FUNC_CALL))
            span.add_queue_item("agent", EvalRecord(context={"query": query}, id="r1"))
        with tracer.start_as_current_span("recipe.generate") as span:
            span.set_attribute("gen_ai.response", json.dumps(GEMINI_TEXT))
            span.add_queue_item("agent", EvalRecord(context={"query": query}, id="r2"))
        return "Steak recipe"

    results = EvalOrchestrator(queue=queue, scenarios=scenarios, agent_fn=mock_adk).run()

    instrumentor.uninstrument()
    assert results.metrics.passed_scenarios == 1
    assert results.metrics.total_scenarios == 1
