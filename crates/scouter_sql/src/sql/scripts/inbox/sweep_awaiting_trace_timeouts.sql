-- Bind: $1 = INTERVAL (e.g. '5 minutes'::interval)
UPDATE scouter.agent_eval_record
SET status = 'failed',
    updated_at = now(),
    context = jsonb_set(COALESCE(context, '{}'::jsonb), '{error}', '"TraceArrivalTimeout"'::jsonb)
WHERE status = 'awaiting_trace'
  AND created_at < now() - $1;
