INSERT INTO scouter.observability_metric (entity_id, request_count, error_count, route_metrics)
VALUES ($1, $2, $3, $4)
ON CONFLICT DO NOTHING;