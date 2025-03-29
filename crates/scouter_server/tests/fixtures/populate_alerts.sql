INSERT INTO scouter.drift_alert (created_at, name, repository, version, feature, alert, active, drift_type, updated_at) VALUES 
(now(), 'model_1', 'repo_1', '1.0.0', 'feature_1', '{"type": "PSI_ALERT"}', true, 'PSI', now()),
(now(), 'model_1', 'repo_1', '1.0.1', 'feature_2', '{"type": "SPC_ALERT"}', true, 'SPC', now()),
(now(), 'model_2', 'repo_2', '2.0.0', 'feature_1', '{"type": "CUSTOM_ALERT"}', false, 'CUSTOM', now()),
(now(), 'model_2', 'repo_2', '2.1.0', 'feature_2', '{"type": "PSI_ALERT"}', true, 'PSI', now()),
(now(), 'model_3', 'repo_3', '1.1.0', 'feature_1', '{"type": "SPC_ALERT"}', true, 'SPC', now()),
(now(), 'model_3', 'repo_3', '1.2.0', 'feature_2', '{"type": "CUSTOM_ALERT"}', false, 'CUSTOM', now()),
(now(), 'model_4', 'repo_4', '2.0.0', 'feature_1', '{"type": "PSI_ALERT"}', true, 'PSI', now()),
(now(), 'model_4', 'repo_4', '2.1.0', 'feature_2', '{"type": "SPC_ALERT"}', true, 'SPC', now()),
(now(), 'model_5', 'repo_5', '3.0.0', 'feature_1', '{"type": "CUSTOM_ALERT"}', true, 'CUSTOM', now()),
(now(), 'model_5', 'repo_5', '3.1.0', 'feature_2', '{"type": "PSI_ALERT"}', false, 'PSI', now());