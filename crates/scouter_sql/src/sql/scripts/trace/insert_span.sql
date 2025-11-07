INSERT INTO scouter.spans (
    created_at,
    span_id, 
    trace_id, 
    parent_span_id, 
    space, 
    name, 
    version, 
    scope,
    span_name, 
    span_kind, 
    start_time, 
    end_time, 
    duration_ms, 
    status_code, 
    status_message, 
    attributes, 
    events, 
    links
)
SELECT 
    created_at,
    span_id, 
    trace_id, 
    parent_span_id, 
    space, 
    name, 
    version, 
    scope,
    span_name, 
    span_kind, 
    start_time, 
    end_time, 
    duration_ms, 
    status_code, 
    status_message, 
    attributes, 
    events, 
    links
FROM UNNEST(
    $1::timestamptz[],  -- created_at
    $2::text[],        -- span_id
    $3::text[],        -- trace_id
    $4::text[],        -- parent_span_id (nullable)
    $5::text[],        -- space
    $6::text[],        -- name
    $7::text[],        -- version
    $8::text[],        -- scope
    $9::text[],        -- span_name
    $10::text[],      -- span_kind
    $11::timestamptz[], -- start_time
    $12::timestamptz[], -- end_time
    $13::bigint[],     -- duration_ms
    $14::text[],       -- status_code
    $15::text[],       -- status_message
    $16::jsonb[],      -- attributes
    $18::jsonb[],      -- events
    $18::jsonb[]       -- links
) AS s(
    created_at,
    span_id, 
    trace_id, 
    parent_span_id, 
    space, 
    name, 
    version, 
    scope,
    span_name, 
    span_kind, 
    start_time, 
    end_time, 
    duration_ms, 
    status_code, 
    status_message, 
    attributes, 
    events, 
    links
)
ON CONFLICT (created_at, span_id, trace_id) DO NOTHING;