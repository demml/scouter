SELECT profile
FROM scouter.drift_profile
WHERE name = $1
  and repository = $2
  and version = $3;