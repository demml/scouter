-- Bind: $1 = bigint[] event ids
-- Bind: $2 = int max_attempts
-- Bind: $3 = text last_error
-- Bind: $4 = uuid claim_token issued by the originating claim
UPDATE scouter.trace_commit_event
SET status = CASE
        WHEN attempt_count >= $2 THEN 'dead_lettered'
        ELSE 'pending'
    END,
    last_error  = $3,
    claimed_at  = NULL,
    claimed_by  = NULL,
    claim_token = NULL,
    updated_at  = now()
WHERE id          = ANY($1)
  AND status      = 'processing'
  AND claim_token = $4
RETURNING id, status;
