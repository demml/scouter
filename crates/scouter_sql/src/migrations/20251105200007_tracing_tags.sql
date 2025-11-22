-- Tables for traces, spans, and baggage in Scouter tracing module
CREATE TABLE IF NOT EXISTS scouter.traces (
    -- non-injected fields
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    trace_id TEXT NOT NULL,
    service_name TEXT,
    space TEXT NOT NULL,
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    scope TEXT NOT NULL,
    trace_state TEXT,
    start_time TIMESTAMPTZ NOT NULL,
    end_time TIMESTAMPTZ,
    duration_ms BIGINT,
    status_code INTEGER DEFAULT 0,
    status_message TEXT,
    root_span_id TEXT,
    span_count INTEGER DEFAULT 0,
    archived BOOLEAN DEFAULT FALSE,
    PRIMARY KEY (created_at, trace_id, scope),
    UNIQUE (created_at, trace_id, space, name, version)
) PARTITION BY RANGE (created_at);


-- Trace indexes
CREATE INDEX idx_traces_entity_lookup
ON scouter.traces (space, name, version, created_at DESC);

CREATE INDEX idx_traces_created_at
ON scouter.traces (created_at DESC, space, name, version);

CREATE INDEX idx_traces_status_time
ON scouter.traces (status_code, created_at DESC)
WHERE status_code != 2;

CREATE INDEX idx_traces_duration_analysis
ON scouter.traces (space, name, version, duration_ms DESC)
WHERE duration_ms IS NOT NULL;

CREATE INDEX idx_traces_time_covering
ON scouter.traces (created_at DESC, space, name, version)
INCLUDE (trace_id, start_time, end_time, duration_ms, status_code, span_count);

CREATE INDEX idx_traces_entity_covering
ON scouter.traces (space, name, version, created_at DESC)
INCLUDE (trace_id, start_time, end_time, duration_ms, status_code, span_count);

-- Spans table - stores individual span data
CREATE TABLE IF NOT EXISTS scouter.spans (
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    span_id TEXT NOT NULL,
    trace_id TEXT NOT NULL,
    parent_span_id TEXT,
    space TEXT NOT NULL,
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    scope TEXT NOT NULL,
    span_name TEXT NOT NULL,
    span_kind TEXT NOT NULL DEFAULT 'internal', -- server, client, producer, consumer, internal
    start_time TIMESTAMPTZ NOT NULL,
    end_time TIMESTAMPTZ,
    duration_ms BIGINT,
    status_code INTEGER DEFAULT 0,
    status_message TEXT,
    attributes JSONB DEFAULT '[]',
    events JSONB DEFAULT '[]',
    links JSONB DEFAULT '[]',
    input JSONB,
    output JSONB,
    label TEXT,
    archived BOOLEAN DEFAULT FALSE,
    PRIMARY KEY (created_at, trace_id, span_id)
) PARTITION BY RANGE (created_at);

CREATE INDEX idx_spans_trace_hierarchy
ON scouter.spans (trace_id, parent_span_id, start_time);

CREATE INDEX idx_spans_entity_lookup
ON scouter.spans (space, name, version, created_at DESC);

CREATE INDEX idx_spans_label_lookup
ON scouter.spans (label, created_at DESC);

CREATE INDEX idx_spans_time_trace
ON scouter.spans (created_at DESC, trace_id);

CREATE INDEX idx_spans_operation_performance
ON scouter.spans (span_name, span_kind, duration_ms DESC)
WHERE duration_ms IS NOT NULL;

CREATE INDEX idx_spans_error_analysis
ON scouter.spans (space, name, version, status_code, created_at DESC)
WHERE status_code != 2;

CREATE INDEX idx_spans_parent_child
ON scouter.spans (parent_span_id, span_id)
WHERE parent_span_id IS NOT NULL;


CREATE TABLE IF NOT EXISTS scouter.trace_baggage (
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    trace_id TEXT NOT NULL,
    scope TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (created_at, trace_id, scope, key)
) PARTITION BY RANGE (created_at);
;

CREATE INDEX idx_baggage_key_lookup
ON scouter.trace_baggage (key, created_at DESC);

CREATE INDEX idx_baggage_trace_scope
ON scouter.trace_baggage (trace_id, scope, created_at DESC);


SELECT scouter.create_parent(
    'scouter.traces',
    'created_at',
    '1 day'
);

SELECT scouter.create_parent(
    'scouter.spans',
    'created_at',
    '1 day'
);

SELECT scouter.create_parent(
    'scouter.trace_baggage',
    'created_at',
    '1 day'
);

CREATE TABLE IF NOT EXISTS scouter.tags (
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (created_at, entity_type, entity_id, key)
) PARTITION BY RANGE (created_at);


CREATE INDEX idx_tags_entity_lookup
ON scouter.tags (entity_type, entity_id, created_at DESC);

CREATE INDEX idx_tags_key_lookup
ON scouter.tags (key, created_at DESC);

CREATE INDEX idx_tags_key_value ON scouter.tags (key, value);

SELECT scouter.create_parent(
    'scouter.tags',
    'created_at',
    '7 days'
);


UPDATE scouter.part_config SET retention = '30 days'
WHERE parent_table IN ('scouter.traces', 'scouter.spans', 'scouter.trace_baggage');

CREATE MATERIALIZED VIEW IF NOT EXISTS scouter.trace_summary AS
SELECT
    t.trace_id,
    t.space,
    t.name,
    t.version,
    t.scope,
    span_stats.start_time,
    span_stats.end_time,
    EXTRACT(EPOCH FROM (span_stats.end_time - span_stats.start_time)) * 1000 AS duration_ms,
    t.status_code,
    t.status_message,
    t.span_count,
    t.created_at,
    t.service_name,
    root_span.span_name as root_operation,
    root_span.span_kind as root_span_kind,
    CASE WHEN t.status_code != 2 THEN true ELSE false END as has_errors,
    COALESCE(error_spans.error_count, 0) as error_count,
    span_stats.avg_span_duration,
    span_stats.max_span_duration
FROM scouter.traces t
LEFT JOIN scouter.spans root_span ON (
    t.trace_id = root_span.trace_id
    AND t.root_span_id = root_span.span_id
    AND root_span.created_at >= NOW() - INTERVAL '7 days'
)
LEFT JOIN (
    SELECT
        trace_id,
        COUNT(*) as error_count
    FROM scouter.spans
    WHERE status_code != 2
    AND created_at >= NOW() - INTERVAL '7 days'
    GROUP BY trace_id
) error_spans ON t.trace_id = error_spans.trace_id
LEFT JOIN (
    SELECT
        trace_id,
        min(start_time) as start_time,
        max(end_time) as end_time,
        AVG(duration_ms) as avg_span_duration,
        MAX(duration_ms) as max_span_duration,
        SUM(duration_ms) as total_span_duration
    FROM scouter.spans
    WHERE duration_ms IS NOT NULL
    AND created_at >= NOW() - INTERVAL '7 days'
    GROUP BY trace_id
) span_stats ON t.trace_id = span_stats.trace_id
WHERE t.created_at >= NOW() - INTERVAL '7 days';

CREATE UNIQUE INDEX IF NOT EXISTS idx_trace_summary_unique
ON scouter.trace_summary (trace_id, scope);

CREATE INDEX IF NOT EXISTS idx_trace_summary_recent
ON scouter.trace_summary (space, name, version, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_trace_summary_service_time
ON scouter.trace_summary (service_name, start_time DESC);

SELECT cron.schedule('refresh-trace-summary', '*/5 * * * *',
    $$REFRESH MATERIALIZED VIEW CONCURRENTLY scouter.trace_summary$$);


CREATE OR REPLACE FUNCTION scouter.get_trace_metrics(
    p_space TEXT DEFAULT NULL,
    p_name TEXT DEFAULT NULL,
    p_version TEXT DEFAULT NULL,
    p_start_time TIMESTAMPTZ DEFAULT NOW() - INTERVAL '1 hour',
    p_end_time TIMESTAMPTZ DEFAULT NOW(),
    p_bucket_interval INTERVAL DEFAULT '5 minutes'
)
RETURNS TABLE (
    bucket_start TIMESTAMPTZ,
    trace_count BIGINT,
    avg_duration_ms FLOAT8,
    p50_duration_ms FLOAT8,
    p95_duration_ms FLOAT8,
    p99_duration_ms FLOAT8,
    error_rate FLOAT8
)
LANGUAGE SQL
STABLE
AS $$
    SELECT
        date_bin(
            p_bucket_interval,
            ts.start_time,
            '2000-01-01 00:00:00'::TIMESTAMPTZ
        ) as bucket_start,
        COUNT(*) as trace_count,
        AVG(ts.duration_ms) as avg_duration_ms,
        PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY ts.duration_ms) as p50_duration_ms,
        PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY ts.duration_ms) as p95_duration_ms,
        PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY ts.duration_ms) as p99_duration_ms,
        (
            COUNT(*) FILTER (WHERE ts.has_errors = true) * 100.0 / COUNT(*)
        ) as error_rate
    FROM scouter.trace_summary ts
    WHERE
        (p_space IS NULL OR ts.space = p_space)
        AND (p_name IS NULL OR ts.name = p_name)
        AND (p_version IS NULL OR ts.version = p_version)
        AND ts.start_time >= p_start_time
        AND ts.start_time <= p_end_time
    GROUP BY bucket_start
    ORDER BY bucket_start DESC;
$$;


CREATE OR REPLACE FUNCTION scouter.get_traces_paginated(
    p_space TEXT DEFAULT NULL,
    p_name TEXT DEFAULT NULL,
    p_version TEXT DEFAULT NULL,
    p_service_name TEXT DEFAULT NULL,
    p_has_errors BOOLEAN DEFAULT NULL,
    p_status_code INTEGER DEFAULT NULL,
    p_start_time TIMESTAMPTZ DEFAULT NOW() - INTERVAL '24 hours',
    p_end_time TIMESTAMPTZ DEFAULT NOW(),
    p_limit INTEGER DEFAULT 50,
    p_cursor_created_at TIMESTAMPTZ DEFAULT NULL,
    p_cursor_trace_id TEXT DEFAULT NULL
)
RETURNS TABLE (
    trace_id TEXT,
    space TEXT,
    name TEXT,
    version TEXT,
    scope TEXT,
    service_name TEXT,
    root_operation TEXT,
    start_time TIMESTAMPTZ,
    end_time TIMESTAMPTZ,
    duration_ms BIGINT,
    status_code INTEGER,
    status_message TEXT,
    span_count INTEGER,
    has_errors BOOLEAN,
    error_count BIGINT,
    created_at TIMESTAMPTZ
)
LANGUAGE SQL
STABLE
AS $$
    SELECT
        ts.trace_id,
        ts.space,
        ts.name,
        ts.version,
        ts.scope,
        ts.service_name,
        ts.root_operation,
        ts.start_time,
        ts.end_time,
        ts.duration_ms,
        ts.status_code,
        ts.status_message,
        ts.span_count,
        ts.has_errors,
        ts.error_count,
        ts.created_at
    FROM scouter.trace_summary ts
    WHERE

        (p_space IS NULL OR ts.space = p_space)
        AND (p_name IS NULL OR ts.name = p_name)
        AND (p_version IS NULL OR ts.version = p_version)
        AND (p_service_name IS NULL OR ts.service_name = p_service_name)
        AND (p_has_errors IS NULL OR ts.has_errors = p_has_errors)
        AND (p_status_code IS NULL OR ts.status_code = p_status_code)
        AND ts.start_time >= p_start_time
        AND ts.start_time <= p_end_time
        AND (
            p_cursor_created_at IS NULL OR
            ts.created_at < p_cursor_created_at OR
            (ts.created_at = p_cursor_created_at AND ts.trace_id < p_cursor_trace_id)
        )
    ORDER BY ts.created_at DESC, ts.trace_id DESC
    LIMIT p_limit + 1;
$$;

CREATE OR REPLACE FUNCTION scouter.get_trace_spans(p_trace_id TEXT)
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
    span_order INTEGER
)
LANGUAGE SQL
STABLE
AS $$
    WITH RECURSIVE span_tree AS (

        SELECT
            s.trace_id,
            s.span_id,
            s.parent_span_id,
            s.span_name,
            s.span_kind,
            s.start_time,
            s.end_time,
            s.duration_ms,
            s.status_code,
            s.status_message,
            s.attributes,
            s.events,
            s.links,
            0 as depth,
            ARRAY[s.span_id] as path,
            s.span_id as root_span_id,
            s.input,
            s.output
        FROM scouter.spans s
        WHERE s.trace_id = p_trace_id
          AND s.parent_span_id IS NULL
        
        UNION ALL
        
        SELECT
            s.trace_id,
            s.span_id,
            s.parent_span_id,
            s.span_name,
            s.span_kind,
            s.start_time,
            s.end_time,
            s.duration_ms,
            s.status_code,
            s.status_message,
            s.attributes,
            s.events,
            s.links,
            st.depth + 1,
            st.path || s.span_id,
            st.root_span_id,
            s.input,
            s.output
        FROM scouter.spans s
        INNER JOIN span_tree st ON s.parent_span_id = st.span_id
        WHERE s.trace_id = p_trace_id AND st.depth < 20
    )
    SELECT
        st.trace_id,
        st.span_id,
        st.parent_span_id,
        st.span_name,
        st.span_kind,
        st.start_time,
        st.end_time,
        st.duration_ms,
        st.status_code,
        st.status_message,
        st.attributes,
        st.events,
        st.links,
        st.depth,
        st.path,
        st.root_span_id,
        st.input,
        st.output,
        ROW_NUMBER() OVER (ORDER BY path) as span_order
    FROM span_tree st
    ORDER BY path;
$$;