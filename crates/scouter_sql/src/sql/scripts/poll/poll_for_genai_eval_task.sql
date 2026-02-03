WITH selected_task AS (
    SELECT
        uid,
        entity_id,
        created_at,
        context,
        retry_count,
        scheduled_at
    FROM scouter.genai_eval_record
    WHERE 1=1
        AND status = 'pending'
        AND (retry_count IS NULL OR retry_count < 3)
        AND scheduled_at <= CURRENT_TIMESTAMP
    ORDER BY
        COALESCE(retry_count, 0) ASC,
        scheduled_at ASC
    LIMIT 1
    FOR UPDATE SKIP LOCKED
)

UPDATE scouter.genai_eval_record dp
SET
    status = 'processing',
    processing_started_at = CURRENT_TIMESTAMP,
    retry_count = COALESCE(dp.retry_count, 0) + 1
FROM selected_task
WHERE dp.uid = selected_task.uid
RETURNING dp.*;