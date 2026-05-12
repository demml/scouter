-- Bind: $1 = bytea[] trace_ids
-- Bind: $2 = bytea[] span_ids
-- Bind: $3 = text[] record_uids
-- Bind: $4 = text[] profile_uids
INSERT INTO scouter.trace_commit_event (trace_id, span_id, record_uid, profile_uid)
SELECT trace_id, span_id, record_uid, profile_uid
FROM unnest($1::bytea[], $2::bytea[], $3::text[], $4::text[])
    AS t(trace_id, span_id, record_uid, profile_uid)
ON CONFLICT ON CONSTRAINT trace_commit_event_record_span_unique DO NOTHING;
