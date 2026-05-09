-- Bind: $1 = LIMIT (e.g. 500)
SELECT id, trace_id
FROM scouter.trace_commit_event
WHERE processed_at IS NULL
ORDER BY id ASC
LIMIT $1
FOR UPDATE SKIP LOCKED;
