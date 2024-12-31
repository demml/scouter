INSERT INTO scouter.custom_metric_record (created_at, name, repository, version, metric, value)
VALUES ($1, $2, $3, $4, $5, $6)
ON CONFLICT DO NOTHING;

