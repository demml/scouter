UPDATE scouter.llm_drift_record
SET status       = $1,
    processing_ended_at = CURRENT_TIMESTAMP,
    updated_at   = CURRENT_TIMESTAMP
WHERE uid= $2
  AND status = 'processing'