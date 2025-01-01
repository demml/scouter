-- update drift profile given name, repository and version

UPDATE drift_profile
SET profile = $1,
    drift_type = $2
WHERE name = $3
  and repository = $4
  and version = $5;