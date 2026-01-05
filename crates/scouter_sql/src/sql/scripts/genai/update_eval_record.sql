UPDATE scouter.genai_eval_record
SET status       = $1,
    processing_ended_at = CURRENT_TIMESTAMP,
    updated_at   = CURRENT_TIMESTAMP,
    processing_duration = $2
WHERE uid = $3
  AND status = 'processing'