UPDATE scouter.drift_profile
SET previous_run = next_run,
    next_run     = $1,
    status       = 'pending',
    processing_started_at = null,
    updated_at   = CURRENT_TIMESTAMP
WHERE entity_id = $2;