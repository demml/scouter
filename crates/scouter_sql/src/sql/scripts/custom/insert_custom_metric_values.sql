INSERT INTO scouter.custom_metric (created_at, name, space, version, metric, value)
VALUES ($1, $2, $3, $4, $5, $6)
ON CONFLICT DO NOTHING;

