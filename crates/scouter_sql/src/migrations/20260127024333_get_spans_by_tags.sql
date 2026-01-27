-- Add migration script here
CREATE OR REPLACE FUNCTION scouter.get_spans_by_tags(
    p_entity_type TEXT,
    p_tag_filters JSONB,
    p_match_all BOOLEAN DEFAULT TRUE,
    p_service_name TEXT DEFAULT NULL
)
RETURNS TABLE (
    trace_id TEXT,
    span_id TEXT,
    parent_span_id TEXT,
    span_name TEXT,
    span_kind TEXT,
    start_time TIMESTAMPTZ,
    end_time TIMESTAMPTZ,
    duration_ms BIGINT,
    status_code INTEGER,
    status_message TEXT,
    attributes JSONB,
    events JSONB,
    links JSONB,
    depth INTEGER,
    path TEXT[],
    root_span_id TEXT,
    input JSONB,
    output JSONB,
    service_name TEXT,
    span_order INTEGER
)
LANGUAGE SQL
STABLE
AS $$
    SELECT s.*
    FROM scouter.search_entities_by_tags(p_entity_type, p_tag_filters, p_match_all) t
    CROSS JOIN LATERAL scouter.get_trace_spans(t.entity_id, p_service_name) s;
$$;