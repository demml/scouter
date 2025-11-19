SELECT 
    uid,
    created_at, 
    name, 
    space, 
    version, 
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
  AND space = $1
  AND name = $2
  AND version = $3