-- Bind: $1 = interval lease_ttl
-- Bind: $2 = int max_attempts
UPDATE scouter.trace_commit_event
SET status = CASE
        WHEN attempt_count >= $2 THEN 'dead_lettered'
        ELSE 'pending'
    END,
    last_error  = COALESCE(last_error, 'ProcessingLeaseExpired'),
    claimed_at  = NULL,
    claimed_by  = NULL,
    claim_token = NULL,
    updated_at  = now()
WHERE status     = 'processing'
  AND claimed_at < now() - $1
RETURNING id, status;
