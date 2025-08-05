SELECT
    id,
    created_at, 
    uid,
    name, 
    space, 
    version, 
    context, 
    prompt, 
    status, 
    score,
    updated_at,
    processing_started_at,
    processing_ended_at,
    processing_duration
FROM scouter.llm_drift_record
WHERE 1=1 
    AND created_at BETWEEN $1 AND $2
    AND space = $3
    AND name = $4
    AND version = $5
    AND archived = false;