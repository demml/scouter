SELECT profile
FROM scouter.drift_profile
WHERE 1=1
  AND entity_id = $1;