INSERT INTO scouter.spc_drift (created_at, entity_id, feature, value)
VALUES ($1, $2, $3, $4)
ON CONFLICT DO NOTHING;