UPDATE scouter.drift_alerts
SET 
    active = $2,
    updated_at = now()
WHERE 
    id = $1
RETURNING 
    id,
    active,
    updated_at;