UPDATE scouter.drift_alert
SET 
    active = $2,
    updated_at = timezone('utc', now())
WHERE 
    id = $1
RETURNING 
    id,
    active,
    updated_at;