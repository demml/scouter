INSERT INTO scouter.drift (created_at, name, space, version, feature, value)
VALUES ($1, $2, $3, $4, $5, $6)
ON CONFLICT DO NOTHING;