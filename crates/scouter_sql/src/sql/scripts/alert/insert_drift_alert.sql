-- insert alerts into alerts
INSERT INTO scouter.drift_alert (
    entity_id,
    entity_name,
    alert,
    drift_type
)
VALUES ($1, $2, $3, $4)
ON CONFLICT (created_at, entity_id) DO NOTHING
RETURNING id, entity_id, entity_name, alert, drift_type, active, created_at, updated_at;