SELECT *
FROM scouter.custom_metric
WHERE created_at < DATEADD(day, -30, CURRENT_TIMESTAMP())
ORDER BY created_at;