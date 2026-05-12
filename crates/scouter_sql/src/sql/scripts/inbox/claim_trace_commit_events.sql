-- Bind: $1 = LIMIT (e.g. 500)
-- Bind: $2 = claimed_by worker id
-- Bind: $3 = claim_token
WITH claimed AS (
    SELECT id
    FROM scouter.trace_commit_event
    WHERE status = 'pending'
    ORDER BY id ASC
    LIMIT $1
    FOR UPDATE SKIP LOCKED
)
UPDATE scouter.trace_commit_event e
SET status        = 'processing',
    claimed_at    = now(),
    claimed_by    = $2,
    claim_token   = $3,
    attempt_count = attempt_count + 1,
    updated_at    = now()
FROM claimed
WHERE e.id = claimed.id
RETURNING e.id, e.trace_id, e.span_id, e.record_uid, e.profile_uid,
          e.attempt_count, e.claim_token;
