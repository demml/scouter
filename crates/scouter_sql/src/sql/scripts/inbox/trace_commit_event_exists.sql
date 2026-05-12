-- Bind: $1 = text record_uid
-- Bind: $2 = bytea trace_id
-- Bind: $3 = bytea span_id
-- Bind: $4 = text profile_uid
SELECT EXISTS (
    SELECT 1
    FROM scouter.trace_commit_event
    WHERE record_uid = $1
      AND trace_id = $2
      AND span_id = $3
      AND profile_uid = $4
) AS exists;
