SELECT id
FROM scouter.entities
WHERE space = $1
    AND name = $2
    AND version = $3;
    and drift_type = $4;