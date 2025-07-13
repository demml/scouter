SELECT 
    created_at,
    space,
    name,
    version,
    input,
    response,
    context,
    prompt
FROM scouter.llm_drift_record
WHERE 1=1
  AND created_at > $4
  AND space = $2
  AND name = $1
  AND version = $3
