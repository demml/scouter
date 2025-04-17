-- update drift profile given name, space and version

UPDATE scouter.drift_profile
SET profile = $1,
    drift_type = $2
WHERE name = $3
  and space = $4
  and version = $5;