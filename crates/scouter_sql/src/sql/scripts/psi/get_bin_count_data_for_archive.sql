SELECT *
FROM scouter.observed_bin_count
WHERE 1=1 
    AND created_at = $1
    AND name = $2
    AND space = $3
    AND version = $4
ORDER BY created_at;