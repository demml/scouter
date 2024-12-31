-- insert alerts into scouter.alerts
INSERT INTO scouter.drift_alerts (name, repository, version, feature, alert)
VALUES ($1, $2, $3, $4, $5)
ON CONFLICT DO NOTHING;