SELECT
    entity_id,
    created_at,
    context,
    prompt,
    status,
    score,
    id,
    updated_at,
    processing_started_at,
    processing_ended_at,
    processing_duration
FROM scouter.llm_drift_record
WHERE 1=1
  AND entity_id = $1