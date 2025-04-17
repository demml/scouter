UPDATE scouter.observed_bin_count
SET 
    archived = true,
    updated_at = timezone('utc', now())
WHERE 
    and space = $1
    and name = $2
    and version = $3;