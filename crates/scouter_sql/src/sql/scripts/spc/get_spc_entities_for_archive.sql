SELECT
    sd.entity_id,
    e.uid as entity_uid,
    MIN(sd.created_at) as begin_timestamp,
    MAX(sd.created_at) as end_timestamp
FROM scouter.spc_drift sd
INNER JOIN scouter.drift_entities e ON sd.entity_id = e.id
WHERE 1=1
    AND sd.created_at < CURRENT_TIMESTAMP - ($1 || ' days')::interval
    AND sd.archived = false
GROUP BY sd.entity_id, e.uid;