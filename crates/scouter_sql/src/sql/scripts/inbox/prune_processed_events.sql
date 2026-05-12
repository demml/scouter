-- Bind: $1 = retention INTERVAL (e.g. '1 day'::interval)
DELETE FROM scouter.trace_commit_event
WHERE status = 'processed'
  AND processed_at < now() - $1;
