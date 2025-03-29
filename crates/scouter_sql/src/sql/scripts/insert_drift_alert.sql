-- insert alerts into alerts
INSERT INTO scouter.drift_alert (name, repository, version, feature, alert, drift_type)
VALUES ($1, $2, $3, $4, $5, $6)
ON CONFLICT DO NOTHING;