SELECT
    id,
    created_at,
    uid,
    context,
    prompt,
    status,
    score,
    updated_at,
    processing_started_at,
    processing_ended_at,
    processing_duration,
    entity_id
FROM scouter.llm_drift_record
WHERE 1=1
    AND created_at BETWEEN $1 AND $2
    AND entity_id = $3
    AND archived = false;