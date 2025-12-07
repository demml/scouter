UPDATE scouter.drift_alert
SET
    active = $2,
    updated_at = CURRENT_TIMESTAMP
WHERE
    id = $1
RETURNING
    id,
    entity_id,
    active,
    updated_at;