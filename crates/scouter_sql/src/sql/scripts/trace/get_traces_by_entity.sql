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
FROM scouter.get_traces_by_entity(
    $1, -- p_entity_uid
    $2, -- p_start_time
    $3, -- p_end_time
    $4, -- p_limit
    $5, -- p_cursor_start_time
    $6  -- p_cursor_trace_id
);