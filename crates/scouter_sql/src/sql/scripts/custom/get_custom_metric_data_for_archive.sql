SELECT *
FROM scouter.custom_metric
WHERE 1=1 
    AND created_at = $1
    AND name = $2
    AND space = $3
    AND version = $4
ORDER BY created_at;