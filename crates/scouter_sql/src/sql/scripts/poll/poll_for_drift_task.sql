WITH selected_task AS (
    SELECT
        uid,
        entity_id,
        profile,
        drift_type,
        previous_run,
        schedule,
        next_run
    FROM scouter.drift_profile
    WHERE active
        AND next_run < CURRENT_TIMESTAMP
        AND status = 'pending'
    ORDER BY next_run ASC
    LIMIT 1
    FOR UPDATE SKIP LOCKED
)

UPDATE scouter.drift_profile dp
SET
    status = 'processing',
    processing_started_at = CURRENT_TIMESTAMP
FROM selected_task
WHERE dp.entity_id = selected_task.entity_id
  AND dp.drift_type = selected_task.drift_type
RETURNING dp.*;