SELECT
    active,
    profile
FROM scouter.drift_profile
WHERE 1=1
    AND space = $1
    AND name = $2
    AND version = $3