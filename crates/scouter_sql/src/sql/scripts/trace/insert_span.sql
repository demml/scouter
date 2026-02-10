-- Refactored insert_span.sql

INSERT INTO scouter.spans (
    created_at,
    span_id,
    trace_id,
    parent_span_id,
    flags,
    trace_state,
    scope_name,
    scope_version,
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
    resource_attributes
)
SELECT * FROM UNNEST(
    $1::timestamptz[], -- created_at
    $2::bytea[], -- span_id
    $3::bytea[], -- trace_id
    $4::bytea[], -- parent_span_id (optional)
    $5::integer[], -- flags
    $6::text[], -- trace_state
    $7::text[], -- scope_name
    $8::text[], -- scope_version (optional)
    $9::text[], -- span_name
    $10::text[], -- span_kind
    $11::timestamptz[], -- start_time
    $12::timestamptz[], -- end_time
    $13::bigint[], -- duration_ms
    $14::integer[], -- status_code
    $15::text[], -- status_message
    $16::jsonb[], -- attributes
    $17::jsonb[], -- events
    $18::jsonb[], -- links
    $19::text[], -- label
    $20::jsonb[], -- input
    $21::jsonb[], -- output
    $22::text[], -- service_name
    $23::jsonb[] -- resource_attributes
)
ON CONFLICT (start_time, trace_id, span_id) DO NOTHING;