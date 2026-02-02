WITH selected_task AS (
    SELECT
        uid,
        entity_id,
        created_at,
        context
    FROM scouter.genai_eval_record
    WHERE 1=1
        AND status = 'pending'
        -- need 5 sec buffer to get traces
        AND created_at <= CURRENT_TIMESTAMP - INTERVAL '5 seconds'
    ORDER BY created_at ASC
    LIMIT 1
    FOR UPDATE SKIP LOCKED
)

UPDATE scouter.genai_eval_record dp
SET
    status = 'processing',
    processing_started_at = CURRENT_TIMESTAMP
FROM selected_task
WHERE dp.uid = selected_task.uid
RETURNING dp.*;