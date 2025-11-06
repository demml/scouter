INSERT INTO scouter.traces (
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
    $1::text[],        -- trace_id
    $2::text[],        -- space
    $3::text[],        -- name
    $4::text[],        -- version
    $5::text[],        -- scope
    $6::text[],        -- trace_state
    $7::timestamptz[], -- start_time
    $8::timestamptz[], -- end_time
    $9::bigint[],      -- duration_ms
    $10::text[],       -- status
    $11::text[],       -- root_span_id
    $12::jsonb[]       -- attributes
) AS t(
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
ON CONFLICT (trace_id, scope) DO UPDATE SET
    end_time = EXCLUDED.end_time,
    duration_ms = EXCLUDED.duration_ms,
    status = EXCLUDED.status,
    span_count = scouter.traces.span_count + 1, 
    trace_state = EXCLUDED.trace_state,
    updated_at = NOW();