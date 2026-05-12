-- Bind: $1 = text[] record_uids
-- Bind: $2 = bytea[] trace_ids
-- Bind: $3 = bytea[] span_ids
-- Bind: $4 = interval trace visibility buffer
WITH completed(record_uid, trace_id, span_id) AS (
    SELECT record_uid, trace_id, span_id
    FROM unnest($1::text[], $2::bytea[], $3::bytea[])
        AS t(record_uid, trace_id, span_id)
)
UPDATE scouter.agent_eval_record AS aer
SET status = 'pending',
    ready_at = now() + $4,
    updated_at = now()
FROM completed
WHERE aer.status = 'awaiting_trace'
  AND aer.uid = completed.record_uid
  AND aer.trace_id = completed.trace_id
  AND aer.span_id = completed.span_id;
