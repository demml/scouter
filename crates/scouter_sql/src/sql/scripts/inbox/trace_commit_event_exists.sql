-- Bind: $1 = bytea (trace_id, length 16)
SELECT EXISTS (
    SELECT 1
    FROM scouter.trace_commit_event
    WHERE trace_id = $1
) AS exists;
