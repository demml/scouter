SELECT *
FROM scouter.observed_bin_count
WHERE created_at < DATEADD(day, -30, CURRENT_TIMESTAMP())
ORDER BY created_at;