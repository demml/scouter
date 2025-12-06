-- Refactored insert_span.sql (FIXED: entity_id removed, service_id calculation added)

-- Insert spans with service_id resolution
INSERT INTO scouter.spans (
    created_at,
    span_id,
    trace_id,
    parent_span_id,
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
    service_id  -- <--- Target Column
)
SELECT
    created_at,
    span_id,
    trace_id,
    parent_span_id,
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
    $5::text[],        -- scope
    $6::text[],        -- span_name
    $7::text[],        -- span_kind
    $8::timestamptz[], -- start_time
    $9::timestamptz[], -- end_time
    $10::bigint[],     -- duration_ms
    $11::integer[],    -- status_code
    $12::text[],       -- status_message
    $13::jsonb[],      -- attributes
    $14::jsonb[],      -- events
    $15::jsonb[],      -- links
    $16::text[],       -- label
    $17::jsonb[],      -- input
    $18::jsonb[],      -- output
    $19::text[]        -- service_name
) AS s(
    created_at,
    span_id,
    trace_id,
    parent_span_id,
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