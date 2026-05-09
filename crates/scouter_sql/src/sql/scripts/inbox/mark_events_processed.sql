-- Bind: $1 = bigint[] (event ids from the claimed batch)
UPDATE scouter.trace_commit_event
SET processed_at = now()
WHERE id = ANY($1);
