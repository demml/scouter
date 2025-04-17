SELECT created_at, space, name, version
FROM scouter.drift
WHERE AND 1=1
 AND created_at < CURRENT_TIMESTAMP - ($1 || ' days')::interval
 AND archived = false
GROUP BY created_at, space, name, version
ORDER BY created_at;