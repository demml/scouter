
SELECT
    id,
    created_at,
    uid,
    entity_id,
    context,
    updated_at,
     status,
    processing_started_at,
    processing_ended_at,
    processing_duration,
    record_id,
    session_id
FROM scouter.genai_eval_record
WHERE 1=1
  AND entity_id = $1