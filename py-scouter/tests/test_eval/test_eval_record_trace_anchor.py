import json

from scouter.evaluate import EvalRecord

TRACE_ID = "00112233445566778899aabbccddeeff"
SPAN_ID = "0123456789abcdef"
OTHER_TRACE_ID = "ffffffffffffffffffffffffffffffff"


class MockSpanContext:
    def __init__(self, trace_id: str = TRACE_ID, span_id: str = SPAN_ID, is_valid: bool = True) -> None:
        self.trace_id = int(trace_id, 16)
        self.span_id = int(span_id, 16)
        self.is_valid = is_valid
        self.trace_flags = 1
        self.is_remote = False
        self.trace_state = {}


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


def test_eval_record_reads_trace_id_and_span_id_from_trace_context() -> None:
    record = EvalRecord(trace_context=MockSpanContext())

    assert record.trace_id == TRACE_ID
    assert record.span_id == SPAN_ID


def test_eval_record_trace_context_wins_over_legacy_trace_id() -> None:
    record = EvalRecord(trace_id=OTHER_TRACE_ID, trace_context=MockSpanContext())

    assert record.trace_id == TRACE_ID
    assert record.span_id == SPAN_ID


def test_eval_record_invalid_trace_context_degrades_to_no_anchor() -> None:
    record = EvalRecord(trace_id=OTHER_TRACE_ID, trace_context=MockSpanContext(is_valid=False))

    assert record.trace_id is None
    assert record.span_id is None


def test_eval_record_profile_uid_sets_entity_uid() -> None:
    record = EvalRecord(profile_uid="p-123")

    assert _dump(record)["entity_uid"] == "p-123"
