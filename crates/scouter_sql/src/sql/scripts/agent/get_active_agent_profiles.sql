SELECT entity_id, profile
FROM scouter.drift_profile
WHERE active = true AND drift_type = 'Agent';
