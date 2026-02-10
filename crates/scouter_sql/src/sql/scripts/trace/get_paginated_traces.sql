-- Updated to accept service_name and resolve to service_id internally
SELECT
    trace_id,
    service_name,
    scope_name,
    scope_version,
    root_operation,
    start_time,
    end_time,
    duration_ms,
    status_code,
    status_message,
    span_count,
    has_errors,
    error_count,
    resource_attributes
FROM scouter.get_traces_paginated(
    $1,  -- p_service_name
    $2,  -- p_has_errors
    $3,  -- p_start_time
    $4,  -- p_end_time
    $5,  -- p_limit
    $6,  -- p_cursor_start_time
    $7,  -- p_cursor_trace_id
    $8,  -- p_direction
    $9,  -- p_attribute_filters
    $10, -- p_trace_ids
    $11  -- p_match_all_attributes
);