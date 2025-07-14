SELECT
    id,
    created_at, 
    uid,
    name, 
    space, 
    version, 
    input, 
    response, 
    context, 
    prompt, 
    status, 
    updated_at,
    processing_started_at,
    processing_ended_at
FROM scouter.llm_drift_record
WHERE 1=1 
    AND created_at BETWEEN $1 AND $2
    AND space = $3
    AND name = $4
    AND version = $5
    AND archived = false;