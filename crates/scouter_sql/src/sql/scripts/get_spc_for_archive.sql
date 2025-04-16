SELECT *
FROM scouter.drift
WHERE created_at < CURRENT_TIMESTAMP - ($1 || ' days')::interval
ORDER BY created_at;