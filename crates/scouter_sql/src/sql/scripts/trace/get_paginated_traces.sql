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
    $1,  -- p_space
    $2,  -- p_name
    $3,  -- p_version
    $4,  -- p_service_name
    $5,  -- p_has_errors
    $6,  -- p_status_code
    $7,  -- p_start_time
    $8,  -- p_end_time
    $9,  -- p_limit
    $10, -- p_cursor_created_at
    $11  -- p_cursor_trace_id
);