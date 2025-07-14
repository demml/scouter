SELECT 
    uid,
    created_at, 
    name, 
    space, 
    version, 
    input, 
    response, 
    context, 
    prompt, 
    status, 
    id
FROM scouter.llm_drift_record 
WHERE 1=1
  AND space = $1
  AND name = $2
  AND version = $3