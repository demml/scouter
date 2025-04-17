SELECT *
FROM scouter.drift
WHERE 1 =1 
    AND created_at < CURRENT_TIMESTAMP - ($1 || ' days')::interval
    AND name = $2
    AND space = $3
    AND version = $4
ORDER BY created_at;