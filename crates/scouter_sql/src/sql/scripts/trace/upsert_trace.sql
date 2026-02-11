INSERT INTO scouter.traces (
    created_at,
    bucket_time,
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
    error_count,
    resource_attributes
)
SELECT * FROM UNNEST(
    $1::timestamptz[],
    $2::timestamptz[],
    $3::bytea[],
    $4::text[],
    $5::text[],
    $6::text[],
    $7::text[],
    $8::timestamptz[],
    $9::timestamptz[],
    $10::bigint[],
    $11::integer[],
    $12::text[],
    $13::bigint[],
    $14::bigint[],
    $15::jsonb[]
)
ON CONFLICT (bucket_time, trace_id)
DO UPDATE SET
    end_time = GREATEST(scouter.traces.end_time, EXCLUDED.end_time),
    start_time = LEAST(scouter.traces.start_time, EXCLUDED.start_time),
    duration_ms = EXTRACT(EPOCH FROM (
        GREATEST(scouter.traces.end_time, EXCLUDED.end_time) -
        LEAST(scouter.traces.start_time, EXCLUDED.start_time)
    )) * 1000,
    status_code = GREATEST(scouter.traces.status_code, EXCLUDED.status_code),
    status_message = COALESCE(
        CASE WHEN EXCLUDED.status_code = 2 THEN EXCLUDED.status_message END,
        scouter.traces.status_message
    ),
    span_count = scouter.traces.span_count + EXCLUDED.span_count,
    error_count = scouter.traces.error_count + EXCLUDED.error_count,
    -- Root-specific metadata: only update if EXCLUDED contains the actual root span
    root_operation = CASE
        WHEN EXCLUDED.root_operation != '' THEN EXCLUDED.root_operation
        ELSE scouter.traces.root_operation
    END,
    service_name = CASE
        WHEN EXCLUDED.root_operation != '' THEN EXCLUDED.service_name
        ELSE scouter.traces.service_name
    END,
    scope_version = CASE
        WHEN EXCLUDED.root_operation != '' THEN EXCLUDED.scope_version
        ELSE scouter.traces.scope_version
    END,
    resource_attributes = CASE
        WHEN EXCLUDED.root_operation != '' THEN EXCLUDED.resource_attributes
        ELSE scouter.traces.resource_attributes
    END;