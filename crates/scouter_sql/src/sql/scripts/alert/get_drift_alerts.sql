SELECT
    id,
    entity_id,
    entity_name,
    alert,
    drift_type,
    active,
    created_at,
    updated_at
FROM scouter.drift_alert
WHERE
    entity_id = $1
    AND ($2 IS NULL OR created_at >= $2)
ORDER BY created_at DESC;