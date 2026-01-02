SELECT
    sd.entity_id,
    MIN(sd.created_at) as begin_timestamp,
    MAX(sd.created_at) as end_timestamp
FROM scouter.genai_eval_task_result sd
WHERE 1=1
    AND sd.created_at < CURRENT_TIMESTAMP - ($1 || ' days')::interval
    AND sd.archived = false
GROUP BY sd.entity_id;