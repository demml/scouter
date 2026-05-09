-- Bind: $1 = bytea[] (trace_ids from the claimed batch)
-- Bind: $2 = interval trace visibility buffer
UPDATE scouter.agent_eval_record
SET status = 'pending',
    ready_at = now() + $2,
    updated_at = now()
WHERE status = 'awaiting_trace'
  AND trace_id = ANY($1);
