INSERT INTO scouter.spans (
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
    $1::text[],        -- span_id
    $2::text[],        -- trace_id
    $3::text[],        -- parent_span_id (nullable)
    $4::text[],        -- space
    $5::text[],        -- name
    $6::text[],        -- version
    $7::text[],        -- scope
    $8::text[],        -- span_name
    $9::text[],        -- span_kind
    $10::timestamptz[], -- start_time
    $11::timestamptz[], -- end_time
    $12::bigint[],     -- duration_ms
    $13::text[],       -- status_code
    $14::text[],       -- status_message
    $15::jsonb[],      -- attributes
    $16::jsonb[],      -- events
    $17::jsonb[]       -- links
) AS s(
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