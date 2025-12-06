-- update drift profile given name, space and version
UPDATE scouter.drift_profile
SET profile = $1,
    drift_type = $2
WHERE 1=1
  AND entity_id = $3