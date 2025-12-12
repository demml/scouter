CREATE TABLE IF NOT EXISTS scouter.trace_baggage (
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    trace_id TEXT NOT NULL,
    scope TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (created_at, trace_id, scope, key)
) PARTITION BY RANGE (created_at);

CREATE INDEX idx_baggage_key_lookup
ON scouter.trace_baggage (key, created_at DESC);

CREATE INDEX idx_baggage_trace_scope
ON scouter.trace_baggage (trace_id, scope, created_at DESC);

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


CREATE TABLE IF NOT EXISTS scouter.spans (
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    span_id TEXT NOT NULL,
    trace_id TEXT NOT NULL,
    parent_span_id TEXT,
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
    resource_attributes JSONB,
    service_id INTEGER,
    service_name TEXT,
    PRIMARY KEY (start_time, trace_id, span_id)
) PARTITION BY RANGE (start_time);



CREATE INDEX idx_spans_trace_lookup ON scouter.spans(trace_id, start_time);
CREATE INDEX idx_spans_service_time ON scouter.spans(service_id, start_time DESC);
CREATE INDEX idx_spans_root ON scouter.spans(trace_id) WHERE parent_span_id IS NULL;
CREATE INDEX IF NOT EXISTS idx_spans_service_id_errors ON scouter.spans (service_id, status_code, start_time DESC)
    WHERE service_id IS NOT NULL AND status_code = 2; -- status code of 2 indicates error in OpenTelemetry
CREATE INDEX IF NOT EXISTS idx_spans_parent_tree ON scouter.spans (parent_span_id, trace_id)
    WHERE parent_span_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_spans_service_name_fallback ON scouter.spans (service_name, start_time DESC)
    WHERE service_name IS NOT NULL;


-- Create partition parents

SELECT scouter.create_parent(
    'scouter.spans',
    'start_time',
    '1 day'
);

UPDATE scouter.part_config
SET
    premake = 7,
    retention = '30 days',
    optimize_constraint = 10,
    retention_keep_table = FALSE
WHERE parent_table = 'scouter.spans';

SELECT scouter.create_parent(
    'scouter.trace_baggage',
    'created_at',
    '1 day'
);

UPDATE scouter.part_config
SET
    premake = 7,
    retention = '30 days',
    retention_keep_table = FALSE
WHERE parent_table = 'scouter.trace_baggage';

SELECT scouter.create_parent(
    'scouter.tags',
    'created_at',
    '7 days'
);

UPDATE scouter.part_config
SET
    premake = 4,
    retention = '90 days',
    retention_keep_table = FALSE
WHERE parent_table = 'scouter.tags';

UPDATE scouter.part_config SET retention_keep_table = FALSE WHERE parent_table = 'scouter.spc_drift';
UPDATE scouter.part_config SET retention_keep_table = FALSE WHERE parent_table = 'scouter.drift_alert';
UPDATE scouter.part_config SET retention_keep_table = FALSE WHERE parent_table = 'scouter.observability_metric';
UPDATE scouter.part_config SET retention_keep_table = FALSE WHERE parent_table = 'scouter.psi_drift';
UPDATE scouter.part_config SET retention_keep_table = FALSE WHERE parent_table = 'scouter.custom_drift';
UPDATE scouter.part_config SET retention_keep_table = FALSE WHERE parent_table = 'scouter.llm_drift';
UPDATE scouter.part_config SET retention_keep_table = FALSE WHERE parent_table = 'scouter.llm_drift_record';


-- common queries (metrics, pagination, span tree)
CREATE OR REPLACE FUNCTION scouter.get_trace_metrics(
    p_service_name TEXT DEFAULT NULL,
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
    WITH service_filter AS (
        SELECT id as service_id
        FROM scouter.service_entities
        WHERE p_service_name IS NULL OR service_name = p_service_name
    ),
    -- Step 1: Reconstruct trace-level timing and status from spans
    trace_metrics AS (
        SELECT
            s.trace_id,

            MIN(s.start_time) as trace_start_time,
            MAX(s.end_time) as trace_end_time,
            -- Calculate total trace duration
            EXTRACT(EPOCH FROM (MAX(s.end_time) - MIN(s.start_time))) * 1000 as duration_ms,
            -- Worst status code across all spans (2 = ERROR in OpenTelemetry)
            MAX(s.status_code) as status_code
        FROM scouter.spans s
        WHERE
            s.start_time >= p_start_time
            AND s.start_time <= p_end_time
            -- Filter by root span service if specified
            AND (p_service_name IS NULL OR EXISTS (
                SELECT 1 FROM scouter.spans root
                WHERE root.trace_id = s.trace_id
                AND root.parent_span_id IS NULL
                AND root.service_id IN (SELECT service_id FROM service_filter)
            ))
        GROUP BY s.trace_id
    ),
    -- Step 2: Bucket traces by time interval and calculate metrics
    bucketed_metrics AS (
        SELECT
            date_bin(
                p_bucket_interval,
                tm.trace_start_time,
                '2000-01-01 00:00:00'::TIMESTAMPTZ
            ) as bucket_start,
            tm.duration_ms,
            tm.status_code
        FROM trace_metrics tm
        WHERE tm.duration_ms IS NOT NULL
    )
    -- Step 3: Aggregate metrics per bucket
    SELECT
        bm.bucket_start,
        COUNT(*) as trace_count,
        AVG(bm.duration_ms)::FLOAT8 as avg_duration_ms,
        PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY bm.duration_ms)::FLOAT8 as p50_duration_ms,
        PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY bm.duration_ms)::FLOAT8 as p95_duration_ms,
        PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY bm.duration_ms)::FLOAT8 as p99_duration_ms,
        (COUNT(*) FILTER (WHERE bm.status_code = 2) / NULLIF(COUNT(*), 0)) * 100.0 as error_rate
    FROM bucketed_metrics bm
    GROUP BY bm.bucket_start
    ORDER BY bm.bucket_start DESC;
$$;


CREATE OR REPLACE FUNCTION scouter.get_traces_paginated(
    p_service_name TEXT DEFAULT NULL,
    p_has_errors BOOLEAN DEFAULT NULL,
    p_status_code INTEGER DEFAULT NULL,
    p_start_time TIMESTAMPTZ DEFAULT NOW() - INTERVAL '24 hours',
    p_end_time TIMESTAMPTZ DEFAULT NOW(),
    p_limit INTEGER DEFAULT 50,
    p_cursor_start_time TIMESTAMPTZ DEFAULT NULL,
    p_cursor_trace_id TEXT DEFAULT NULL,
    p_direction TEXT DEFAULT 'next'
)
RETURNS TABLE (
    trace_id TEXT,
    service_name TEXT,
    scope TEXT,
    root_operation TEXT,
    start_time TIMESTAMPTZ,
    end_time TIMESTAMPTZ,
    duration_ms BIGINT,
    status_code INTEGER,
    status_message TEXT,
    span_count BIGINT,
    has_errors BOOLEAN,
    error_count BIGINT,
    resource_attributes JSONB
)
LANGUAGE SQL
STABLE
AS $$
    WITH service_filter AS (
        SELECT id as service_id
        FROM scouter.service_entities
        WHERE p_service_name IS NULL OR service_name = p_service_name
    ),
    -- Step 1: Aggregate trace-level metadata from spans
    trace_aggregates AS (
    SELECT
        s.trace_id,
        COALESCE(
            (SELECT service_name FROM scouter.spans
             WHERE trace_id = s.trace_id
             AND parent_span_id IS NULL
             LIMIT 1),
            MIN(s.service_name)
        ) as service_name,
        COALESCE(
            (SELECT scope FROM scouter.spans
             WHERE trace_id = s.trace_id
             AND parent_span_id IS NULL
             LIMIT 1),
            MIN(s.scope)
        ) as scope,
        COALESCE(
            (SELECT span_name FROM scouter.spans
             WHERE trace_id = s.trace_id
             AND parent_span_id IS NULL
             LIMIT 1),
            'Unknown Operation'
        ) as root_operation,
        COALESCE(
            (SELECT resource_attributes FROM scouter.spans
             WHERE trace_id = s.trace_id
             AND parent_span_id IS NULL
             LIMIT 1),
            '[]'::JSONB
        ) as resource_attributes,
        MIN(s.start_time) as start_time,
        MAX(s.end_time) as end_time,
        EXTRACT(EPOCH FROM (MAX(s.end_time) - MIN(s.start_time))) * 1000 as duration_ms,
        MAX(s.status_code) as status_code,
        (SELECT status_message FROM scouter.spans
         WHERE trace_id = s.trace_id
         AND status_code = 2
         LIMIT 1) as status_message,
        COUNT(*) as span_count,
        COUNT(*) FILTER (WHERE s.status_code = 2) as error_count
    FROM scouter.spans s
    WHERE
        s.start_time >= p_start_time
        AND s.start_time <= p_end_time
        AND (p_service_name IS NULL OR EXISTS (
            SELECT 1 FROM scouter.spans root
            WHERE root.trace_id = s.trace_id
            AND root.parent_span_id IS NULL
            AND root.service_id IN (SELECT service_id FROM service_filter)
        ))
    GROUP BY s.trace_id
)
    SELECT
        ta.trace_id,
        ta.service_name,
        ta.scope,
        ta.root_operation,
        ta.start_time,
        ta.end_time,
        ta.duration_ms::BIGINT,
        ta.status_code,
        ta.status_message,
        ta.span_count,
        (ta.error_count > 0) as has_errors,
        ta.error_count,
        ta.resource_attributes
    FROM trace_aggregates ta
    WHERE
        -- Error filtering
        (p_has_errors IS NULL
            OR (p_has_errors = true AND ta.error_count > 0)
            OR (p_has_errors = false AND ta.error_count = 0)
        )
        -- Status code filtering
        AND (p_status_code IS NULL OR ta.status_code = p_status_code)
        -- Cursor-based pagination
        AND (
            (p_direction = 'next' AND (
                p_cursor_start_time IS NULL OR
                ta.start_time < p_cursor_start_time OR
                (ta.start_time = p_cursor_start_time AND ta.trace_id < p_cursor_trace_id)
            ))
            OR
            (p_direction = 'previous' AND (
                p_cursor_start_time IS NULL OR
                ta.start_time > p_cursor_start_time OR
                (ta.start_time = p_cursor_start_time AND ta.trace_id > p_cursor_trace_id)
            ))
        )
    ORDER BY
        CASE WHEN p_direction = 'next' THEN ta.start_time END DESC,
        CASE WHEN p_direction = 'next' THEN ta.trace_id END DESC,
        CASE WHEN p_direction = 'previous' THEN ta.start_time END ASC,
        CASE WHEN p_direction = 'previous' THEN ta.trace_id END ASC
    LIMIT p_limit + 1;
$$;

-- 7.3: get_trace_spans
CREATE OR REPLACE FUNCTION scouter.get_trace_spans(
    p_trace_id TEXT,
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
    WITH RECURSIVE service_filter AS (  -- <<<< FIX APPLIED HERE: RECURSIVE MOVED TO THE START
        SELECT id as service_id
        FROM scouter.service_entities
        WHERE p_service_name IS NULL OR service_name = p_service_name
        LIMIT 1
    ),
    span_tree AS (
        SELECT
            s.trace_id, s.span_id, s.parent_span_id, s.span_name, s.span_kind, s.start_time, s.end_time, s.duration_ms,
            s.status_code, s.status_message, s.attributes, s.events, s.links,
            0 as depth,
            ARRAY[s.span_id] as path,
            s.span_id as root_span_id,
            s.input,
            s.output,
            s.service_name
        FROM scouter.spans s
        WHERE s.trace_id = p_trace_id
          AND s.parent_span_id IS NULL
          AND (p_service_name IS NULL OR s.service_id = (SELECT service_id FROM service_filter))

        UNION ALL

        SELECT
            s.trace_id, s.span_id, s.parent_span_id, s.span_name, s.span_kind, s.start_time, s.end_time, s.duration_ms,
            s.status_code, s.status_message, s.attributes, s.events, s.links,
            st.depth + 1,
            st.path || s.span_id,
            st.root_span_id,
            s.input,
            s.output,
            s.service_name
        FROM scouter.spans s
        INNER JOIN span_tree st ON s.parent_span_id = st.span_id
        WHERE s.trace_id = p_trace_id
          AND st.depth < 20
          AND (p_service_name IS NULL OR s.service_id = (SELECT service_id FROM service_filter))
    )
    SELECT
        st.trace_id, st.span_id, st.parent_span_id, st.span_name, st.span_kind, st.start_time, st.end_time, st.duration_ms,
        st.status_code, st.status_message, st.attributes, st.events, st.links,
        st.depth, st.path, st.root_span_id, st.input, st.output, st.service_name,
        ROW_NUMBER() OVER (ORDER BY path) as span_order
    FROM span_tree st
    ORDER BY path;
$$;