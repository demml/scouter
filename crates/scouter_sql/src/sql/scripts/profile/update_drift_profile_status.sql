UPDATE scouter.drift_profile
SET active = $1
WHERE 1=1
  AND space = $3
  AND name = $2
  AND version = $4
  AND ($5 IS NULL OR drift_type = $5);