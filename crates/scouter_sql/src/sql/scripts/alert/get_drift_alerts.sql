SELECT
created_at,
name,
space,
version,
entity_name,
alert,
id,
drift_type,
active
FROM scouter.drift_alert
WHERE
    1=1
    AND ($4 IS NULL OR created_at >= $4)
    AND space = $3
    AND name = $2
    AND version = $1
    