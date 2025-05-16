SELECT profile
FROM scouter.drift_profile
WHERE 1=1
  AND space = $2
  AND name = $1
  AND version = $3
  AND drift_type = $4;