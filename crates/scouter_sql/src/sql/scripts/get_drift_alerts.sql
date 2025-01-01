SELECT
created_at,
name,
repository,
version,
feature,
alert,
id,
status
FROM drift_alerts
WHERE
    version = $1
    AND name = $2
    AND repository = $3