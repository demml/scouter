WITH selected_task AS (
    SELECT
        uid,
        entity_id,
        created_at,
        context,
        prompt,
        score
    FROM scouter.genai_event_record
    WHERE 1=1
        AND status = 'pending'
    ORDER BY created_at ASC
    LIMIT 1
    FOR UPDATE SKIP LOCKED
)

UPDATE scouter.genai_event_record dp
SET
    status = 'processing',
    processing_started_at = CURRENT_TIMESTAMP
FROM selected_task
WHERE dp.uid = selected_task.uid
RETURNING dp.*;