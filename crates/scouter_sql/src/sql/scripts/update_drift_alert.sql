UPDATE scouter.drift_alerts
SET 
    status = $2,
    updated_at = now()
WHERE 
    id = $1
RETURNING 
    id,
    status,
    updated_at;