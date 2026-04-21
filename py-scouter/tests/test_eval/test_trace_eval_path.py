from scouter.drift import (
    AgentEvalConfig,
    AgentEvalProfile,
    AssertionTask,
    ComparisonOperator,
)
from scouter.tracing import (
    SCOUTER_ACTIVE_ENTITY_UID_BAGGAGE_KEY,
    ScouterInstrumentor,
    active_profile,
)


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

    with active_profile(profile):
        value = baggage.get_baggage(
            SCOUTER_ACTIVE_ENTITY_UID_BAGGAGE_KEY,
            context=context_api.get_current(),
        )

    assert value == profile.config.uid


def test_active_profile_clears_on_exit():
    from opentelemetry import baggage
    from opentelemetry import context as context_api

    profile = _make_profile("agent")

    with active_profile(profile):
        pass

    value = baggage.get_baggage(
        SCOUTER_ACTIVE_ENTITY_UID_BAGGAGE_KEY,
        context=context_api.get_current(),
    )
    assert value is None


def test_active_profile_sequential_multi_agent():
    from opentelemetry import baggage
    from opentelemetry import context as context_api

    profile_a = _make_profile("alpha")
    profile_b = _make_profile("beta")

    with active_profile(profile_a):
        uid_a = baggage.get_baggage(
            SCOUTER_ACTIVE_ENTITY_UID_BAGGAGE_KEY,
            context=context_api.get_current(),
        )

    with active_profile(profile_b):
        uid_b = baggage.get_baggage(
            SCOUTER_ACTIVE_ENTITY_UID_BAGGAGE_KEY,
            context=context_api.get_current(),
        )

    assert uid_a == profile_a.config.uid
    assert uid_b == profile_b.config.uid


def test_active_profile_nested():
    from opentelemetry import baggage
    from opentelemetry import context as context_api

    outer = _make_profile("outer")
    inner = _make_profile("inner")

    with active_profile(outer):
        with active_profile(inner):
            active_inside_inner = baggage.get_baggage(
                SCOUTER_ACTIVE_ENTITY_UID_BAGGAGE_KEY,
                context=context_api.get_current(),
            )

        active_after_inner = baggage.get_baggage(
            SCOUTER_ACTIVE_ENTITY_UID_BAGGAGE_KEY,
            context=context_api.get_current(),
        )

    assert active_inside_inner == inner.config.uid
    assert active_after_inner == outer.config.uid


def test_instrument_eval_profiles_single_agent():
    from unittest.mock import patch

    ScouterInstrumentor._instance = None
    ScouterInstrumentor._provider = None

    profile = _make_profile("bot")
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

    assert captured_provider_kwargs["default_entity_uid"] == profile.config.uid

    ScouterInstrumentor._instance = None
    ScouterInstrumentor._provider = None


def test_instrument_eval_profiles_uses_first_profile_as_default():
    from unittest.mock import patch

    ScouterInstrumentor._instance = None
    ScouterInstrumentor._provider = None

    first = _make_profile("first")
    second = _make_profile("second")
    instrumentor = ScouterInstrumentor()
    captured_provider_kwargs: dict = {}

    class _FakeTracerProvider:
        def __init__(self, **kwargs):
            captured_provider_kwargs.update(kwargs)

    with (
        patch("scouter.tracing.TracerProvider", _FakeTracerProvider),
        patch("scouter.tracing.set_tracer_provider"),
    ):
        instrumentor._instrument(eval_profiles=[first, second])

    assert captured_provider_kwargs["default_entity_uid"] == first.config.uid

    ScouterInstrumentor._instance = None
    ScouterInstrumentor._provider = None
