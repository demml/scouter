INSERT INTO scouter.drift_alert (created_at, name, space, version, feature, alert, active, drift_type, updated_at) VALUES 
('2025-01-01 12:00:00'::timestamptz, 'model_1', 'repo_1', '1.0.0', 'feature_1', '{"type": "PSI_ALERT"}', true, 'PSI', now()),
('2025-01-01 11:00:00'::timestamptz, 'model_1', 'repo_1', '1.0.0', 'feature_2', '{"type": "SPC_ALERT"}', true, 'SPC', now()),
('2025-01-01 10:00:00'::timestamptz, 'model_1', 'repo_1', '1.0.0', 'feature_1', '{"type": "CUSTOM_ALERT"}', false, 'CUSTOM', now()),
('2025-01-01 09:00:00'::timestamptz, 'model_1', 'repo_1', '1.0.0', 'feature_2', '{"type": "PSI_ALERT"}', true, 'PSI', now()),
('2025-01-01 08:00:00'::timestamptz, 'model_1', 'repo_1', '1.0.0', 'feature_1', '{"type": "SPC_ALERT"}', true, 'SPC', now()),
('2025-01-01 07:00:00'::timestamptz, 'model_1', 'repo_1', '1.0.0', 'feature_2', '{"type": "CUSTOM_ALERT"}', false, 'CUSTOM', now()),
('2025-01-01 06:00:00'::timestamptz, 'model_1', 'repo_1', '1.0.0', 'feature_1', '{"type": "PSI_ALERT"}', true, 'PSI', now()),
('2025-01-01 05:00:00'::timestamptz, 'model_1', 'repo_1', '1.0.0', 'feature_2', '{"type": "SPC_ALERT"}', true, 'SPC', now()),
('2025-01-01 04:00:00'::timestamptz, 'model_1', 'repo_1', '1.0.0', 'feature_1', '{"type": "CUSTOM_ALERT"}', true, 'CUSTOM', now()),
('2025-01-01 03:00:00'::timestamptz, 'model_1', 'repo_1', '1.0.0', 'feature_2', '{"type": "PSI_ALERT"}', false, 'PSI', now());