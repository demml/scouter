-- Bind: $1 = interval reconcile_after
-- Bind: $2 = bigint limit
SELECT uid, trace_id, span_id, created_at
FROM scouter.agent_eval_record
WHERE status     = 'awaiting_trace'
  AND span_id    IS NOT NULL
  AND trace_id   IS NOT NULL
  AND created_at < now() - $1
ORDER BY created_at ASC
LIMIT $2;
