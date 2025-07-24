UPDATE scouter.llm_drift_record
SET status       = $1,
    score        = $2,
    processing_ended_at = CURRENT_TIMESTAMP,
    updated_at   = CURRENT_TIMESTAMP
WHERE uid= $3
  AND status = 'processing'