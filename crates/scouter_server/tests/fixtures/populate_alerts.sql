INSERT INTO scouter.drift_entities (space, name, version, drift_type) VALUES
('repo_1', 'model_1', '1.0.0', 'PSI'),
('repo_1', 'model_1', '1.0.0', 'SPC'),
('repo_1', 'model_1', '1.0.0', 'CUSTOM')
ON CONFLICT (space, name, version, drift_type) DO NOTHING;

-- Now insert drift alerts using the entity_ids from the entities table
-- Note: We join with entities to get the correct entity_id for each alert
INSERT INTO scouter.drift_alert (created_at, entity_id, entity_name, alert, active, updated_at)
SELECT
    created_at,
    e.id as entity_id,
    entity_name,
    alert,
    active,
    updated_at
FROM (VALUES
    ('2025-01-01 12:00:00'::timestamptz, 'repo_1', 'model_1', '1.0.0', 'PSI', 'feature_1', '{"type": "PSI_ALERT"}', true, now()),
    ('2025-01-01 11:00:00'::timestamptz, 'repo_1', 'model_1', '1.0.0', 'SPC', 'feature_2', '{"type": "SPC_ALERT"}', true, now()),
    ('2025-01-01 10:00:00'::timestamptz, 'repo_1', 'model_1', '1.0.0', 'CUSTOM', 'feature_1', '{"type": "CUSTOM_ALERT"}', false, now()),
    ('2025-01-01 09:00:00'::timestamptz, 'repo_1', 'model_1', '1.0.0', 'PSI', 'feature_2', '{"type": "PSI_ALERT"}', true, now()),
    ('2025-01-01 08:00:00'::timestamptz, 'repo_1', 'model_1', '1.0.0', 'SPC', 'feature_1', '{"type": "SPC_ALERT"}', true, now()),
    ('2025-01-01 07:00:00'::timestamptz, 'repo_1', 'model_1', '1.0.0', 'CUSTOM', 'feature_2', '{"type": "CUSTOM_ALERT"}', false, now()),
    ('2025-01-01 06:00:00'::timestamptz, 'repo_1', 'model_1', '1.0.0', 'PSI', 'feature_1', '{"type": "PSI_ALERT"}', true, now()),
    ('2025-01-01 05:00:00'::timestamptz, 'repo_1', 'model_1', '1.0.0', 'SPC', 'feature_2', '{"type": "SPC_ALERT"}', true, now()),
    ('2025-01-01 04:00:00'::timestamptz, 'repo_1', 'model_1', '1.0.0', 'CUSTOM', 'feature_1', '{"type": "CUSTOM_ALERT"}', true, now()),
    ('2025-01-01 03:00:00'::timestamptz, 'repo_1', 'model_1', '1.0.0', 'PSI', 'feature_2', '{"type": "PSI_ALERT"}', false, now())
) AS v(created_at, space, name, version, drift_type, entity_name, alert, active, updated_at)
INNER JOIN scouter.drift_entities e
    ON e.space = v.space
    AND e.name = v.name
    AND e.version = v.version
    AND e.drift_type = v.drift_type;