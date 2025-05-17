SELECT 
    space, 
    name, 
    version,
    MIN(created_at) as begin_timestamp,
    MAX(created_at) as end_timestamp
FROM scouter.custom_drift
WHERE 1=1
    AND created_at < CURRENT_TIMESTAMP - ($1 || ' days')::interval
    AND archived = false
GROUP BY space, name, version;