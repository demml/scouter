UPDATE scouter.custom_metric
SET 
    archived = true,
    updated_at = timezone('utc', now())
WHERE 1=1 
    AND created_at BETWEEN $1 AND $2
    AND space = $3
    AND name = $4
    AND version = $5
    AND archived = false;