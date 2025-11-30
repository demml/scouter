INSERT INTO scouter.traces (
    created_at,
    trace_id,
    entity_id,
    scope,
    trace_state,
    start_time,
    end_time,
    duration_ms,
    status_code,
    status_message,
    root_span_id,
    span_count
)
SELECT
    created_at,
    trace_id,
    entity_id,
    scope,
    trace_state,
    start_time,
    end_time,
    duration_ms,
    status_code,
    status_message,
    root_span_id,
    span_count
FROM UNNEST(
    $1::timestamptz[],  -- created_at
    $2::text[],        -- trace_id
    $3::integer[],        -- entity_id
    $4::text[],        -- scope
    $5::text[],        -- trace_state
    $6::timestamptz[], -- start_time
    $7::timestamptz[], -- end_time
    $8::bigint[],      -- duration_ms
    $9::integer[],       -- status_code
    $10::text[],       -- status_message
    $11::text[],       -- root_span_id
    $12::integer[]    -- span_count
) AS t(
        created_at,
        trace_id,
        entity_id,
        scope,
        trace_state,
        start_time,
        end_time,
        duration_ms,
        status_code,
        status_message,
        root_span_id,
        span_count
    )
ON CONFLICT (created_at, trace_id, scope) DO UPDATE SET
    -- Only updating fields that can change over time
    end_time = EXCLUDED.end_time,
    duration_ms = EXTRACT(EPOCH FROM (EXCLUDED.end_time - scouter.traces.start_time)) * 1000,
    status_code = EXCLUDED.status_code,
    status_message = EXCLUDED.status_message,
    span_count = scouter.traces.span_count + EXCLUDED.span_count,
    trace_state = EXCLUDED.trace_state,
    updated_at = NOW();