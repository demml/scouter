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
    status,
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
    status, 
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
    $11::text[],       -- status
    $12::text[],       -- root_span_id
    $13::jsonb[]       -- attributes
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
        status, 
        root_span_id, 
        attributes
    )
ON CONFLICT (created_at, trace_id, scope) DO UPDATE SET
    end_time = EXCLUDED.end_time,
    duration_ms = EXCLUDED.duration_ms,
    status = EXCLUDED.status,
    span_count = scouter.traces.span_count + 1, 
    trace_state = EXCLUDED.trace_state,
    updated_at = NOW();