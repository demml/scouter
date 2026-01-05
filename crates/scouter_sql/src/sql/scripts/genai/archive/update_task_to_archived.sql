UPDATE scouter.genai_eval_task
SET
    archived = true,
    updated_at = CURRENT_TIMESTAMP
WHERE 1=1
    AND created_at BETWEEN $1 AND $2
    AND entity_id = $3
    AND archived = false;