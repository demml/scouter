INSERT INTO scouter.observability_metrics (space, name, version, request_count, error_count, route_metrics)
VALUES ($1, $2, $3, $4, $5, $6)
ON CONFLICT DO NOTHING;