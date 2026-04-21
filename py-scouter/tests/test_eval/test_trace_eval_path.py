from scouter.drift import (
    AgentEvalConfig,
    AgentEvalProfile,
    AssertionTask,
    ComparisonOperator,
)
from scouter.tracing import ScouterInstrumentor, active_profile


def _make_task(task_id: str = "check_response") -> AssertionTask:
    return AssertionTask(
        id=task_id,
        context_path="response",
        operator=ComparisonOperator.IsString,
        expected_value=True,
        description="Check response is a string",
    )


def _make_profile(name: str = "agent") -> AgentEvalProfile:
    return AgentEvalProfile(
        tasks=[_make_task()],
        config=AgentEvalConfig(space="test", name=name),
    )


def test_active_profile_sets_baggage():
    from opentelemetry import baggage
    from opentelemetry import context as context_api

    profile = _make_profile("agent")
    key = f"scouter.entity.{profile.config.name}"

    with active_profile(profile):
        value = baggage.get_baggage(key, context=context_api.get_current())

    assert value == profile.config.uid


def test_active_profile_clears_on_exit():
    from opentelemetry import baggage
    from opentelemetry import context as context_api

    profile = _make_profile("agent")
    key = f"scouter.entity.{profile.config.name}"

    with active_profile(profile):
        pass

    value = baggage.get_baggage(key, context=context_api.get_current())
    assert value is None


def test_active_profile_sequential_multi_agent():
    from opentelemetry import baggage
    from opentelemetry import context as context_api

    profile_a = _make_profile("alpha")
    profile_b = _make_profile("beta")

    key_a = f"scouter.entity.{profile_a.config.name}"
    key_b = f"scouter.entity.{profile_b.config.name}"

    with active_profile(profile_a):
        uid_a = baggage.get_baggage(key_a, context=context_api.get_current())
        uid_b_during_a = baggage.get_baggage(key_b, context=context_api.get_current())

    with active_profile(profile_b):
        uid_b = baggage.get_baggage(key_b, context=context_api.get_current())
        uid_a_during_b = baggage.get_baggage(key_a, context=context_api.get_current())

    assert uid_a == profile_a.config.uid
    assert uid_b_during_a is None
    assert uid_b == profile_b.config.uid
    assert uid_a_during_b is None


def test_active_profile_nested():
    from opentelemetry import baggage
    from opentelemetry import context as context_api

    outer = _make_profile("outer")
    inner = _make_profile("inner")

    key_outer = f"scouter.entity.{outer.config.name}"
    key_inner = f"scouter.entity.{inner.config.name}"

    with active_profile(outer):
        with active_profile(inner):
            val_outer_inside_inner = baggage.get_baggage(key_outer, context=context_api.get_current())
            val_inner_inside_inner = baggage.get_baggage(key_inner, context=context_api.get_current())

        val_inner_after_inner = baggage.get_baggage(key_inner, context=context_api.get_current())
        val_outer_after_inner = baggage.get_baggage(key_outer, context=context_api.get_current())

    assert val_outer_inside_inner == outer.config.uid
    assert val_inner_inside_inner == inner.config.uid
    assert val_inner_after_inner is None
    assert val_outer_after_inner == outer.config.uid


def test_instrument_eval_profiles_single_agent():
    from unittest.mock import patch

    ScouterInstrumentor._instance = None
    ScouterInstrumentor._provider = None

    profile = _make_profile("bot")
    key = f"scouter.entity.{profile.config.name}"

    instrumentor = ScouterInstrumentor()

    captured_provider_kwargs: dict = {}

    class _FakeTracerProvider:
        def __init__(self, **kwargs):
            captured_provider_kwargs.update(kwargs)

    with (
        patch("scouter.tracing.TracerProvider", _FakeTracerProvider),
        patch("scouter.tracing.set_tracer_provider"),
    ):
        instrumentor._instrument(eval_profiles=[profile])

    attrs = captured_provider_kwargs["default_attributes"]
    assert key in attrs
    assert attrs[key] == profile.config.uid

    ScouterInstrumentor._instance = None
    ScouterInstrumentor._provider = None
