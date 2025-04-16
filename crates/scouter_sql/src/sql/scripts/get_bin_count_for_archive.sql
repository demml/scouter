SELECT *
FROM scouter.observed_bin_count
WHERE created_at < CURRENT_TIMESTAMP - ($1 || ' days')::interval
ORDER BY created_at;