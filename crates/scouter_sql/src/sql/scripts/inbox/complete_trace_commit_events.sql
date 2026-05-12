-- Bind: $1 = bigint[] event ids drained in this batch
-- Bind: $2 = uuid claim_token issued by the originating claim
UPDATE scouter.trace_commit_event
SET status       = 'processed',
    processed_at = now(),
    claim_token  = NULL,
    updated_at   = now()
WHERE id          = ANY($1)
  AND status      = 'processing'
  AND claim_token = $2
RETURNING id, trace_id, span_id, record_uid, profile_uid;
