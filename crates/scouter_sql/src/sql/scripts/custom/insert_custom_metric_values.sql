INSERT INTO scouter.custom_drift (created_at, entity_id, metric, value)
VALUES ($1, $2, $3, $4)
ON CONFLICT DO NOTHING;

