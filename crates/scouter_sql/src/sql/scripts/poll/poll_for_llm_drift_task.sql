WITH selected_task AS (
    SELECT 
        uid,
        created_at, 
        space, 
        name,
        version, 
        input, 
        response, 
        context, 
        prompt
    FROM scouter.llm_drift_record
    WHERE 1=1 
        AND status = 'pending'
    ORDER BY created_at ASC
    LIMIT 1 
    FOR UPDATE SKIP LOCKED
)

UPDATE scouter.llm_drift_record dp
SET 
    status = 'processing',
    processing_started_at = CURRENT_TIMESTAMP
FROM selected_task
WHERE dp.uid = selected_task.uid
RETURNING dp.*;