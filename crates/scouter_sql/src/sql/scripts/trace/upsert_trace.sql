INSERT INTO scouter.traces (
    created_at,
    trace_id,
    space,
    name,
    version,
    scope,
    trace_state,
    start_time,
    end_time,
    duration_ms,
    status_code,
    status_message,
    root_span_id,
    span_count,
    attributes
)
SELECT
    created_at,
    trace_id,
    space,
    name,
    version,
    scope,
    trace_state,
    start_time,
    end_time,
    duration_ms,
    status_code,
    status_message,
    root_span_id,
    1 as span_count,
    attributes
FROM UNNEST(
    $1::timestamptz[],  -- created_at
    $2::text[],        -- trace_id
    $3::text[],        -- space
    $4::text[],        -- name
    $5::text[],        -- version
    $6::text[],        -- scope
    $7::text[],        -- trace_state
    $8::timestamptz[], -- start_time
    $9::timestamptz[], -- end_time
    $10::bigint[],      -- duration_ms
    $11::integer[],       -- status_code
    $12::text[],       -- status_message
    $13::text[],       -- root_span_id
    $14::jsonb[]       -- attributes
) AS t(
        created_at,
        trace_id,
        space,
        name,
        version,
        scope,
        trace_state,
        start_time,
        end_time,
        duration_ms,
        status_code,
        status_message,
        root_span_id,
        attributes
    )
ON CONFLICT (created_at, trace_id, scope) DO UPDATE SET
    -- Only updating fields that can change over time
    end_time = EXCLUDED.end_time,
    duration_ms = EXTRACT(EPOCH FROM (EXCLUDED.end_time - scouter.traces.start_time)) * 1000,
    status_code = EXCLUDED.status_code,
    status_message = EXCLUDED.status_message,
    attributes = scouter.traces.attributes || EXCLUDED.attributes,
    span_count = scouter.traces.span_count + 1,
    trace_state = EXCLUDED.trace_state,
    updated_at = NOW();