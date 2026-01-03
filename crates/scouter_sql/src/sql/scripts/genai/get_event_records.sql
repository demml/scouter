
SELECT
    uid,
    created_at,
    context,
    status,
    id,
    updated_at,
    processing_started_at,
    processing_ended_at,
    processing_duration,
    entity_id
FROM scouter.genai_event_record
WHERE 1=1
  AND entity_id = $1