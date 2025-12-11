-- Updated to accept service_name and resolve to service_id internally
SELECT
    trace_id,
    service_name,
    scope,
    root_operation,
    start_time,
    end_time,
    duration_ms,
    status_code,
    status_message,
    span_count,
    has_errors,
    error_count,
    created_at,
    resource_attributes
FROM scouter.get_traces_paginated(
    $1,  -- p_service_name
    $2,  -- p_has_errors
    $3,  -- p_status_code
    $4,  -- p_start_time
    $5,  -- p_end_time
    $6,  -- p_limit
    $7,  -- p_cursor_created_at
    $8,  -- p_cursor_trace_id
    $9  -- p_direction
);