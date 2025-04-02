SELECT
created_at,
name,
space,
version,
feature,
alert,
id,
status
FROM scouter.drift_alerts
WHERE
    1=1
    AND version = $1
    AND name = $2
    AND space = $3
    AND ($4 IS NULL OR created_at >= $4::timestamp)
    