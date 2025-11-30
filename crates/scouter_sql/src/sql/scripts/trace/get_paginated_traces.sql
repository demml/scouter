SELECT
    trace_id,
    space,
    name,
    version,
    scope,
    service_name,
    root_operation,
    start_time,
    end_time,
    duration_ms,
    status_code,
    status_message,
    span_count,
    has_errors,
    error_count,
    created_at
FROM scouter.get_traces_paginated(
    $1,  -- p_entity_id
    $2,  -- p_service_name
    $3,  -- p_has_errors
    $4,  -- p_status_code
    $5,  -- p_start_time
    $6,  -- p_end_time
    $7,  -- p_limit
    $8, -- p_cursor_created_at
    $9, -- p_cursor_trace_id
    $10  -- p_direction
);