SELECT
    trace_id,
    span_id,
    parent_span_id,
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
    depth,
    path,
    root_span_id,
    input,
    output,
    service_name,
    span_order
FROM scouter.get_spans_by_tags(
    $1, -- p_entity_type
    $2, -- p_tag_filters (JSONB)
    $3, -- p_match_all
    $4  -- p_service_name
);