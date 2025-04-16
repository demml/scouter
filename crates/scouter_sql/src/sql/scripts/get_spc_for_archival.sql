SELECT *
FROM scouter.drift
WHERE created_at < DATEADD(day, -30, CURRENT_TIMESTAMP())
ORDER BY created_at;