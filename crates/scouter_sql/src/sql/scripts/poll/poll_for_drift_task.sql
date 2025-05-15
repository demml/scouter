WITH selected_task AS (
    SELECT 
        uid,
        name, 
        space, 
        version, 
        profile, 
        drift_type, 
        previous_run, 
        schedule,
        next_run
    FROM scouter.drift_profile
    WHERE active 
        AND next_run < CURRENT_TIMESTAMP
        AND status = 'pending'  -- Add status column
    ORDER BY next_run ASC
    LIMIT 1 
    FOR UPDATE SKIP LOCKED
)

UPDATE scouter.drift_profile dp
SET 
    status = 'processing',
    processing_started_at = CURRENT_TIMESTAMP
FROM selected_task
WHERE dp.uid = selected_task.uid
RETURNING dp.*;