-- Bind: $1 = bytea[] (trace_ids from the claimed batch)
UPDATE scouter.agent_eval_record
SET status = 'pending', updated_at = now()
WHERE status = 'awaiting_trace'
  AND trace_id = ANY($1);
