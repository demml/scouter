SELECT *
FROM scouter.custom_metric
WHERE created_at < CURRENT_TIMESTAMP - ($1 || ' days')::interval
ORDER BY created_at;