-- Generic paginated traces query with optional tag filtering
-- Pass NULL or empty JSONB array for $1 to skip tag filtering
WITH matching_traces AS (
    SELECT entity_id as trace_id
    FROM scouter.search_entities_by_tags(
        'trace',
        $1,  -- p_tag_filters (JSONB: [{"key": "environment", "value": "production"}] or NULL/[]::jsonb)
        $2  -- p_match_all (defaults to true if NULL)
    )
    WHERE $1 IS NOT NULL AND jsonb_array_length($1) > 0  -- Only execute if filters provided
)
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
    resource_attributes
FROM scouter.get_traces_paginated(
    $3,  -- p_service_name
    $4,  -- p_has_errors
    $5,  -- p_status_code
    $6,  -- p_start_time
    $7,  -- p_end_time
    $8,  -- p_limit
    $9,  -- p_cursor_start_time
    $10, -- p_cursor_trace_id
    $11, -- p_direction
    CASE 
        WHEN $1 IS NOT NULL AND jsonb_array_length($1) > 0
        THEN ARRAY(SELECT trace_id FROM matching_traces)
        ELSE NULL
    END  -- p_trace_ids (NULL means no filtering)
);