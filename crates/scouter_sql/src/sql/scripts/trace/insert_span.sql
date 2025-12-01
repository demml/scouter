-- Insert spans with service_id resolution
INSERT INTO scouter.spans (
    created_at,
    span_id,
    trace_id,
    parent_span_id,
    entity_id,
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
    links,
    label,
    input,
    output,
    service_name,
    service_id
)
SELECT
    created_at,
    span_id,
    trace_id,
    parent_span_id,
    entity_id,
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
    links,
    label,
    input,
    output,
    service_name,
    scouter.get_or_create_service_id(service_name) as service_id
FROM UNNEST(
    $1::timestamptz[],  -- created_at
    $2::text[],        -- span_id
    $3::text[],        -- trace_id
    $4::text[],        -- parent_span_id (nullable)
    $5::integer[],     -- entity_id
    $6::text[],        -- scope
    $7::text[],        -- span_name
    $8::text[],        -- span_kind
    $9::timestamptz[], -- start_time
    $10::timestamptz[], -- end_time
    $11::bigint[],     -- duration_ms
    $12::integer[],    -- status_code
    $13::text[],       -- status_message
    $14::jsonb[],      -- attributes
    $15::jsonb[],      -- events
    $16::jsonb[],      -- links
    $17::text[],       -- label
    $18::jsonb[],      -- input
    $19::jsonb[],      -- output
    $20::text[]        -- service_name
) AS s(
    created_at,
    span_id,
    trace_id,
    parent_span_id,
    entity_id,
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
    links,
    label,
    input,
    output,
    service_name
)
ON CONFLICT (created_at, trace_id, span_id) DO NOTHING;