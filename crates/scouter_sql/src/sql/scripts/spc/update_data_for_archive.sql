UPDATE scouter.drift
SET 
    archived = true,
    updated_at = timezone('utc', now())
WHERE 
    and space = $1
    and name = $2
    and version = $3;