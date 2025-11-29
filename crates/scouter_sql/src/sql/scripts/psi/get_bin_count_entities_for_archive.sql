SELECT
    entity_id,
    MIN(created_at) as begin_timestamp,
    MAX(created_at) as end_timestamp
FROM scouter.psi_drift
WHERE 1=1
    AND created_at < CURRENT_TIMESTAMP - ($1 || ' hours')::interval
    AND archived = false
GROUP BY entity_id;