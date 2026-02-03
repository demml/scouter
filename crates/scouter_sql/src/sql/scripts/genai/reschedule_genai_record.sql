UPDATE scouter.genai_eval_record
SET
    status = 'pending',
    scheduled_at = $1,
    processing_started_at = NULL,
    updated_at = CURRENT_TIMESTAMP
WHERE uid = $2
    AND status = 'processing';