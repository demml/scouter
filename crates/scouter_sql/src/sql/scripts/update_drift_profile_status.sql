UPDATE scouter.drift_profile
SET active = $1
WHERE name = $2
  and repository = $3
  and version = $4;
  AND ($5 IS NULL OR drift_type = $5)