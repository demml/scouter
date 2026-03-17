import pytest
from scouter.drift import GenAIEvalProfile
from scouter.evaluate import (
    AssertionTask,
    ComparisonOperator,
    EvalOrchestrator,
    EvalRecord,
    EvalScenario,
    EvalScenarios,
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


# ---------------------------------------------------------------------------
# Unit Tests
# ---------------------------------------------------------------------------
class TestDefaultExecution:
    def test_single_turn(self):
        """Default execute_agent calls agent_fn with initial_query."""
        profile = _simple_profile()
        scenarios = _simple_scenarios(["What is 2+2?"])
        queue = _make_queue(profile)

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
                    EvalRecord(
                        context={"response": {"quality": 9, "text": "4"}},
                        id="rec_1",
                    ),
                )
            return "4"

        orch = EvalOrchestrator(queue=queue, scenarios=scenarios, agent_fn=my_agent)
        results = orch.run()

        assert call_log == ["What is 2+2?"]
        assert isinstance(results, ScenarioEvalResults)
        assert results.metrics.total_scenarios == 1
        assert results.metrics.passed_scenarios == 1

    def test_multi_turn(self):
        """Default execute_agent calls agent_fn for initial_query + each predefined_turn."""
        profile = _simple_profile()
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
        queue = _make_queue(profile)

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

        orch = EvalOrchestrator(queue=queue, scenarios=scenarios, agent_fn=my_agent)
        results = orch.run()

        assert call_log == ["Plan dinner", "Make it vegetarian"]
        assert results.metrics.total_scenarios == 1


class TestSubclassWithoutAgentFn:
    def test_subclass_override(self):
        """Subclass overrides execute_agent — no agent_fn needed."""
        profile = _simple_profile()
        scenarios = _simple_scenarios(["What is 2+2?"])
        queue = _make_queue(profile)

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
                        EvalRecord(
                            context={"response": {"quality": 9, "text": "4"}},
                            id="rec_1",
                        ),
                    )
                return "4"

        orch = MyOrchestrator(queue=queue, scenarios=scenarios)
        results = orch.run()

        assert isinstance(results, ScenarioEvalResults)
        assert results.metrics.total_scenarios == 1
        assert results.metrics.passed_scenarios == 1

    def test_no_agent_fn_no_override_raises(self):
        """NotImplementedError when neither agent_fn nor override is provided."""
        profile = _simple_profile()
        scenarios = _simple_scenarios(["Q1"])
        queue = _make_queue(profile)

        orch = EvalOrchestrator(queue=queue, scenarios=scenarios)

        with pytest.raises(NotImplementedError, match="agent_fn"):
            orch.run()


class TestReactiveScenario:
    def test_reactive_raises(self):
        """Reactive scenario (simulated_user_persona set) raises NotImplementedError."""
        profile = _simple_profile()
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
        queue = _make_queue(profile)

        orch = EvalOrchestrator(
            queue=queue, scenarios=scenarios, agent_fn=lambda q: "response"
        )

        with pytest.raises(NotImplementedError, match="Reactive"):
            orch.run()


class TestHookOrdering:
    def test_hook_order(self):
        """Verify: on_scenario_start -> execute -> on_scenario_complete -> on_evaluation_complete."""
        profile = _simple_profile()
        scenarios = _simple_scenarios(["Q1"])
        queue = _make_queue(profile)

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

        orch = HookOrchestrator(queue=queue, scenarios=scenarios)
        orch.run()

        assert hook_log[0] == ("on_scenario_start", "scenario_1")
        assert hook_log[1] == ("execute_agent", "scenario_1")
        assert hook_log[2] == ("on_scenario_complete", "scenario_1", "answer")
        assert hook_log[3] == ("on_evaluation_complete",)


class TestCaptureLifecycle:
    def test_capture_cleanup_on_exception(self):
        """enable_capture/disable_capture called even when agent_fn raises."""
        profile = _simple_profile()
        scenarios = _simple_scenarios(["Q1"])
        queue = _make_queue(profile)

        def failing_agent(query):
            raise RuntimeError("agent failed")

        orch = EvalOrchestrator(
            queue=queue, scenarios=scenarios, agent_fn=failing_agent
        )

        with pytest.raises(RuntimeError, match="agent failed"):
            orch.run()

        queue.disable_capture()


# ---------------------------------------------------------------------------
# ADK-style integration test
# ---------------------------------------------------------------------------

# ── GenAI eval profiles (one per sub-agent) ───────────────────────────────

retriever_eval_profile = GenAIEvalProfile(
    tasks=[
        AssertionTask(
            id="has_results",
            context_path="results.count",
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=1,
        ),
        TraceAssertionTask(
            id="retriever_span",
            assertion=TraceAssertion.span_count(
                SpanFilter.by_name("retriever_callback")
            ),
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=1,
        ),
    ],
    alias="retriever",
)

synthesizer_eval_profile = GenAIEvalProfile(
    tasks=[
        AssertionTask(
            id="quality_score",
            context_path="response.quality",
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=7,
        ),
        TraceAssertionTask(
            id="synthesizer_span",
            assertion=TraceAssertion.span_count(
                SpanFilter.by_name("synthesizer_callback")
            ),
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=1,
        ),
    ],
    alias="synthesizer",
)

# ── Scenarios ─────────────────────────────────────────────────────────────

EVAL_SCENARIOS = EvalScenarios(
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


# ── Mock sub-agent callbacks (ADK after_model_callback pattern) ───────────

# Simulated data each sub-agent "produces" per query
RETRIEVER_DATA = {
    "What is retrieval augmented generation?": {"count": 5, "source": "arxiv"},
    "How do transformer attention heads work?": {"count": 3, "source": "papers"},
    "Compare BERT and GPT architectures": {"count": 4, "source": "textbook"},
}

SYNTHESIZER_DATA = {
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


class TestADKAgentPattern:
    """Full ADK-style integration: profiles → queue → ScouterInstrumentor → agents → eval → save/load → compare."""

    @pytest.fixture(autouse=True)
    def _setup(self):
        """Set up ScouterInstrumentor + queue for each test."""
        self.queue = ScouterQueue.from_profile(
            profile=[retriever_eval_profile, synthesizer_eval_profile],
            transport_config=MockConfig(),
            wait_for_startup=True,
        )
        self.instrumentor = ScouterInstrumentor()
        self.instrumentor.instrument(
            scouter_queue=self.queue,
            exporter=TestSpanExporter(batch_export=False),
        )
        # Get a tracer from the instrumented provider (mirrors ADK pattern:
        # after instrument(), sub-agent callbacks use get_tracer/start_as_current_span)
        self.tracer = init_tracer(
            service_name="adk-agent",
            scouter_queue=self.queue,
            transport_config=MockConfig(),
            exporter=TestSpanExporter(batch_export=False),
        )

        yield

        self.instrumentor.uninstrument()

    # ── Mock sub-agent callbacks (ADK after_model_callback pattern) ───

    def _retriever_callback(self, query):
        """Sub-agent callback: retriever logs results via a traced span."""
        with self.tracer.start_as_current_span("retriever_callback") as span:
            data = RETRIEVER_DATA[query]
            span.add_queue_item(
                "retriever",
                EvalRecord(
                    context={
                        "results": {"count": data["count"], "source": data["source"]}
                    },
                    id=f"retriever_{query[:10]}",
                ),
            )
        return data

    def _synthesizer_callback(self, query, data_override=None):
        """Sub-agent callback: synthesizer logs quality + text via a traced span."""
        with self.tracer.start_as_current_span("synthesizer_callback") as span:
            data = data_override or SYNTHESIZER_DATA[query]
            span.add_queue_item(
                "synthesizer",
                EvalRecord(
                    context={
                        "response": {"quality": data["quality"], "text": data["text"]}
                    },
                    id=f"synthesizer_{query[:10]}",
                ),
            )
        return data

    def _make_agent_fn(self, synth_overrides=None):
        """Create the root agent function that orchestrates sub-agents."""

        def agent_fn(query):
            with self.tracer.start_as_current_span("orchestrator_call"):
                ret_data = self._retriever_callback(query)
                override = synth_overrides.get(query) if synth_overrides else None
                syn_data = self._synthesizer_callback(query, data_override=override)
            return f"[{ret_data['count']} sources] {syn_data['text']}"

        return agent_fn

    def _run_eval(self, synth_overrides=None):
        """Helper: run orchestrator eval, optionally overriding synthesizer data."""
        orch = EvalOrchestrator(
            queue=self.queue,
            scenarios=EVAL_SCENARIOS,
            agent_fn=self._make_agent_fn(synth_overrides),
        )
        return orch.run()

    def test_baseline_eval(self):
        """Run baseline eval: 3 scenarios, 2 sub-agents, all pass."""
        results = self._run_eval()

        assert results.metrics.total_scenarios == 3
        assert results.metrics.passed_scenarios == 3
        assert "retriever" in results.metrics.dataset_pass_rates
        assert "synthesizer" in results.metrics.dataset_pass_rates

        ret = results.dataset_results["retriever"]
        assert ret.successful_count + ret.failed_count == 3

        syn = results.dataset_results["synthesizer"]
        assert syn.successful_count + syn.failed_count == 3

    def test_save_load_roundtrip(self, tmp_path):
        """save() → load() produces identical results."""
        results = self._run_eval()

        path = str(tmp_path / "baseline.json")
        results.save(path)
        loaded = ScenarioEvalResults.load(path)

        assert loaded.metrics.total_scenarios == results.metrics.total_scenarios
        assert loaded.metrics.passed_scenarios == results.metrics.passed_scenarios
        assert loaded.metrics.overall_pass_rate == pytest.approx(
            results.metrics.overall_pass_rate
        )
        assert len(loaded.scenario_results) == len(results.scenario_results)

    def test_compare_baseline_to_improved(self, tmp_path):
        """Run baseline, run improved, compare — no regression."""
        baseline = self._run_eval()
        baseline.save(str(tmp_path / "baseline.json"))

        improved = self._run_eval(
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
            }
        )
        improved.save(str(tmp_path / "improved.json"))

        comp = improved.compare_to(baseline)
        assert not comp.regressed
        assert comp.comparison_overall_pass_rate >= comp.baseline_overall_pass_rate

        # Roundtrip the comparison
        comp.save(str(tmp_path / "comparison.json"))
        from scouter.evaluate import ScenarioComparisonResults

        loaded_comp = ScenarioComparisonResults.load(str(tmp_path / "comparison.json"))
        assert loaded_comp.regressed == comp.regressed

    def test_compare_baseline_to_regressed(self):
        """Run baseline, run regressed, compare — regression detected."""
        baseline = self._run_eval()

        regressed = self._run_eval(
            synth_overrides={
                "What is retrieval augmented generation?": {
                    "quality": 3,
                    "text": "bad",
                },
                "How do transformer attention heads work?": {
                    "quality": 2,
                    "text": "worse",
                },
                "Compare BERT and GPT architectures": {
                    "quality": 4,
                    "text": "terrible",
                },
            }
        )

        comp = regressed.compare_to(baseline)
        assert comp.regressed
        assert comp.comparison_overall_pass_rate < comp.baseline_overall_pass_rate
