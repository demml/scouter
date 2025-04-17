SELECT created_at, space, name, version
FROM scouter.drift
WHERE created_at < CURRENT_TIMESTAMP - ($1 || ' days')::interval
GROUP BY created_at, space, name, version
ORDER BY created_at;