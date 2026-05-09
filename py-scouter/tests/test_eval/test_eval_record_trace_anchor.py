import json
from typing import Any

import pytest
from scouter.evaluate import EvalRecord

TRACE_ID = "00112233445566778899aabbccddeeff"


def _dump(record: EvalRecord) -> dict:
    return json.loads(record.model_dump_json())


def test_eval_record_without_trace_context_or_trace_id_has_no_anchor() -> None:
    record = EvalRecord(context={"input": "hello"})

    assert record.trace_id is None
    assert record.span_id is None


def test_eval_record_accepts_legacy_trace_id_without_span_id() -> None:
    record = EvalRecord(trace_id=TRACE_ID)

    assert record.trace_id == TRACE_ID
    assert record.span_id is None


def test_eval_record_uses_record_id_not_public_id() -> None:
    record = EvalRecord(record_id="turn-7")

    assert record.record_id == "turn-7"


def test_eval_record_rejects_public_id_alias() -> None:
    kwargs: dict[str, Any] = {"id": "turn-7"}
    with pytest.raises(TypeError):
        EvalRecord(**kwargs)


def test_eval_record_rejects_public_trace_context_kwarg() -> None:
    kwargs: dict[str, Any] = {"trace_context": object()}
    with pytest.raises(TypeError):
        EvalRecord(**kwargs)


def test_eval_record_accepts_tags() -> None:
    record = EvalRecord(tags=["run_id=abc", "scenario_id=s1"])

    assert record.tags == ["run_id=abc", "scenario_id=s1"]


def test_eval_record_profile_uid_sets_entity_uid() -> None:
    record = EvalRecord(profile_uid="p-123")

    assert record.entity_uid == "p-123"
    assert _dump(record)["entity_uid"] == "p-123"
