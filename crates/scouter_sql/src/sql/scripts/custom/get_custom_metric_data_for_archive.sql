SELECT *
FROM scouter.custom_drift
WHERE 1=1
    AND created_at BETWEEN $1 AND $2
    AND entity_id = $3
    AND archived = false;