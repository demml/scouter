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
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    PRIMARY KEY (entity_type, entity_id, key, value)
);

-- Optimized indexes for common query patterns
CREATE INDEX idx_tags_key_value ON scouter.tags (key, value);
CREATE INDEX idx_tags_entity_type ON scouter.tags (entity_type);
CREATE INDEX idx_tags_updated_at ON scouter.tags (updated_at DESC);

CREATE INDEX idx_tags_entity_lookup
ON scouter.tags (entity_type, entity_id, created_at DESC);

CREATE INDEX idx_tags_key_lookup
ON scouter.tags (key, created_at DESC);

-- Partial index for scouter queue trace lookups
CREATE INDEX idx_tags_genai_queue_record
ON scouter.tags (entity_id, value)
WHERE entity_type = 'trace'
  AND key = 'scouter.queue.record';


CREATE TABLE IF NOT EXISTS scouter.spans (
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    trace_id BYTEA NOT NULL CHECK (octet_length(trace_id) = 16),
    span_id BYTEA NOT NULL CHECK (octet_length(span_id) = 8),
    parent_span_id BYTEA CHECK (parent_span_id IS NULL OR octet_length(parent_span_id) = 8),
    flags INTEGER DEFAULT 1 NOT NULL,
    scope_name TEXT NOT NULL,
    scope_version TEXT,
    trace_state TEXT,
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

CREATE INDEX idx_spans_root_partitioned
ON scouter.spans(trace_id, start_time)
WHERE parent_span_id IS NULL;

CREATE INDEX IF NOT EXISTS idx_spans_service_id_errors ON scouter.spans (service_id, status_code, start_time DESC)
    WHERE service_id IS NOT NULL AND status_code = 2; -- status code of 2 indicates error in OpenTelemetry
CREATE INDEX IF NOT EXISTS idx_spans_parent_tree ON scouter.spans (parent_span_id, trace_id)
    WHERE parent_span_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_spans_service_name_fallback ON scouter.spans (service_name, start_time DESC)
    WHERE service_name IS NOT NULL;

CREATE INDEX idx_spans_attributes_containment
ON scouter.spans USING GIN (attributes jsonb_path_ops);

CREATE INDEX idx_spans_tree_traversal
ON scouter.spans (trace_id, parent_span_id, start_time)
INCLUDE (span_name, span_kind, duration_ms, status_code);


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

UPDATE scouter.part_config SET retention_keep_table = FALSE WHERE parent_table = 'scouter.spc_drift';
UPDATE scouter.part_config SET retention_keep_table = FALSE WHERE parent_table = 'scouter.drift_alert';
UPDATE scouter.part_config SET retention_keep_table = FALSE WHERE parent_table = 'scouter.observability_metric';
UPDATE scouter.part_config SET retention_keep_table = FALSE WHERE parent_table = 'scouter.psi_drift';
UPDATE scouter.part_config SET retention_keep_table = FALSE WHERE parent_table = 'scouter.custom_drift';
UPDATE scouter.part_config SET retention_keep_table = FALSE WHERE parent_table = 'scouter.llm_drift';
UPDATE scouter.part_config SET retention_keep_table = FALSE WHERE parent_table = 'scouter.llm_drift_record';


-- trace aggregations
CREATE TABLE IF NOT EXISTS scouter.traces (
    -- Truncated start_time (hourly) to act as partition key
    bucket_time TIMESTAMPTZ NOT NULL, -- need bucket for reconciling distributed traces
    trace_id BYTEA NOT NULL CHECK (octet_length(trace_id) = 16),

    -- Metadata (Aggregated)
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    service_name TEXT,
    scope_name TEXT,
    scope_version TEXT,
    root_operation TEXT,
    start_time TIMESTAMPTZ NOT NULL,
    end_time TIMESTAMPTZ,
    duration_ms BIGINT,
    status_code INTEGER DEFAULT 0,
    status_message TEXT,
    span_count BIGINT DEFAULT 0,
    error_count BIGINT DEFAULT 0,
    resource_attributes JSONB DEFAULT '{}',
    PRIMARY KEY (bucket_time, trace_id)
) PARTITION BY RANGE (bucket_time);

CREATE INDEX idx_traces_id_lookup ON scouter.traces (trace_id);
ALTER TABLE scouter.traces SET (fillfactor = 80);
ALTER TABLE scouter.traces SET (
  autovacuum_vacuum_scale_factor = 0.01,
  autovacuum_analyze_scale_factor = 0.005,
  autovacuum_vacuum_cost_limit = 1000
);

SELECT scouter.create_parent(
    'scouter.traces',
    'bucket_time',
    '1 day'
);

UPDATE scouter.part_config
SET premake = 7, retention = '30 days'
WHERE parent_table = 'scouter.traces';

CREATE OR REPLACE FUNCTION scouter.match_span_attributes(
    span_attributes JSONB,
    attribute_filters JSONB,
    match_all BOOLEAN
)
RETURNS BOOLEAN
LANGUAGE SQL
IMMUTABLE
AS $$
    SELECT
        CASE WHEN match_all THEN
            (
                SELECT bool_and(
                    EXISTS (
                        SELECT 1
                        FROM jsonb_array_elements(span_attributes) AS attr
                        WHERE (attr->>'key') = (filter->>'key')
                        AND (attr->>'value')::text = (filter->>'value')::text
                    )
                )
                FROM jsonb_array_elements(attribute_filters) AS filter
            )
        ELSE
            EXISTS (
                SELECT 1
                FROM jsonb_array_elements(attribute_filters) AS filter
                WHERE EXISTS (
                    SELECT 1
                    FROM jsonb_array_elements(span_attributes) AS attr
                    WHERE (attr->>'key') = (filter->>'key')
                    AND (attr->>'value')::text = (filter->>'value')::text
                )
            )
        END
$$;

CREATE OR REPLACE FUNCTION scouter.get_trace_metrics(
    p_service_name TEXT DEFAULT NULL,
    p_start_time TIMESTAMPTZ DEFAULT NOW() - INTERVAL '1 hour',
    p_end_time TIMESTAMPTZ DEFAULT NOW(),
    p_bucket_interval INTERVAL DEFAULT '5 minutes',
    p_attribute_filters JSONB DEFAULT NULL,
    p_match_all_attributes BOOLEAN DEFAULT FALSE
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

    matching_traces AS (
        SELECT DISTINCT trace_id
        FROM scouter.spans
        WHERE
            start_time >= p_start_time
            AND start_time <= p_end_time
            AND (p_attribute_filters IS NULL OR
                scouter.match_span_attributes(
                    attributes,
                    p_attribute_filters,
                    p_match_all_attributes
                )
            )
    ),
    trace_metrics AS (
        SELECT
            s.trace_id,
            MIN(s.start_time) as trace_start_time,
            MAX(s.end_time) as trace_end_time,
            EXTRACT(EPOCH FROM (MAX(s.end_time) - MIN(s.start_time))) * 1000 as duration_ms,
            MAX(s.status_code) as status_code
        FROM scouter.spans s
        WHERE
            s.start_time >= p_start_time
            AND s.start_time <= p_end_time
            -- Only include traces that have matching spans
            AND (p_attribute_filters IS NULL OR s.trace_id IN (SELECT trace_id FROM matching_traces))
            -- Apply service filter to root spans
            AND (p_service_name IS NULL OR EXISTS (
                SELECT 1 FROM scouter.spans root
                WHERE root.trace_id = s.trace_id
                AND root.parent_span_id IS NULL
                AND root.service_id IN (SELECT service_id FROM service_filter)
            ))
        GROUP BY s.trace_id
    ),
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
    p_direction TEXT DEFAULT 'next',
    p_trace_ids TEXT[] DEFAULT NULL,
    p_attribute_filters JSONB DEFAULT NULL,
    p_match_all_attributes BOOLEAN DEFAULT FALSE
)
RETURNS TABLE (
    trace_id TEXT,
    service_name TEXT,
    scope_name TEXT,
    scope_version TEXT,
    root_operation TEXT,
    start_time TIMESTAMPTZ,
    end_time TIMESTAMPTZ,
    duration_ms BIGINT,
    status_code INTEGER,
    status_message TEXT,
    span_count BIGINT,
    has_errors BOOLEAN,
    error_count BIGINT,
    resource_attributes JSONB,
    attributes JSONB
)
LANGUAGE SQL
STABLE
AS $$
    WITH filtered_trace_ids AS (
        SELECT DISTINCT s.trace_id
        FROM scouter.spans s
        WHERE p_attribute_filters IS NOT NULL
          AND s.start_time >= p_start_time
          AND s.start_time <= p_end_time
          AND scouter.match_span_attributes(
                s.attributes,
                p_attribute_filters,
                p_match_all_attributes
          )
    ),
    base_query AS (
        SELECT
            t.trace_id,
            t.service_name,
            t.scope_name,
            t.scope_version,
            t.root_operation,
            t.start_time,
            t.end_time,
            t.duration_ms,
            t.status_code,
            t.status_message,
            t.span_count,
            (t.error_count > 0) as has_errors,
            t.error_count,
            t.resource_attributes,
            -- Root attributes are pulled from spans only for the current page to save IO
            COALESCE(
                (SELECT s.attributes FROM scouter.spans s
                 WHERE s.trace_id = t.trace_id
                 AND s.parent_span_id IS NULL
                 AND s.start_time >= t.start_time - INTERVAL '1 minute'
                 AND s.start_time <= t.start_time + INTERVAL '1 minute'
                 LIMIT 1),
                '[]'::JSONB
            ) as attributes
        FROM scouter.traces t
        WHERE
            -- CRITICAL: Partition Pruning using bucket_time
            t.bucket_time >= date_trunc('hour', p_start_time)
            AND t.bucket_time <= date_trunc('hour', p_end_time)

            -- Basic Filters
            AND (p_trace_ids IS NULL OR t.trace_id = ANY(p_trace_ids::bytea[]))
            AND (p_service_name IS NULL OR t.service_name = p_service_name)
            AND (p_status_code IS NULL OR t.status_code = p_status_code)
            AND (p_has_errors IS NULL OR (p_has_errors = (t.error_count > 0)))
            AND (p_attribute_filters IS NULL OR t.trace_id IN (SELECT trace_id FROM filtered_trace_ids))
            AND (
                CASE
                    WHEN p_direction = 'next' THEN
                        (p_cursor_start_time IS NULL OR
                         t.start_time < p_cursor_start_time OR
                         (t.start_time = p_cursor_start_time AND t.trace_id < decode(p_cursor_trace_id, 'hex')))
                    WHEN p_direction = 'previous' THEN
                        (p_cursor_start_time IS NULL OR
                         t.start_time > p_cursor_start_time OR
                         (t.start_time = p_cursor_start_time AND t.trace_id > decode(p_cursor_trace_id, 'hex')))
                END
            )
        ORDER BY
            CASE WHEN p_direction = 'next' THEN t.start_time END DESC,
            CASE WHEN p_direction = 'next' THEN t.trace_id END DESC,
            CASE WHEN p_direction = 'previous' THEN t.start_time END ASC,
            CASE WHEN p_direction = 'previous' THEN t.trace_id END ASC
        LIMIT p_limit + 1
    )
    SELECT
        encode(bq.trace_id, 'hex'),
        bq.service_name,
        bq.scope_name,
        bq.scope_version,
        bq.root_operation,
        bq.start_time,
        bq.end_time,
        bq.duration_ms,
        bq.status_code,
        bq.status_message,
        bq.span_count,
        bq.has_errors,
        bq.error_count,
        bq.resource_attributes,
        bq.attributes
    FROM base_query bq;
$$;


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
    WITH RECURSIVE service_filter AS (
        SELECT id as service_id
        FROM scouter.service_entities
        WHERE p_service_name IS NULL OR service_name = p_service_name
        LIMIT 1
    ),
    span_tree AS (
        SELECT
            encode(s.trace_id, 'hex') as trace_id,
            encode(s.span_id, 'hex') as span_id,
            encode(s.parent_span_id, 'hex') as parent_span_id,
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
            s.output,
            s.service_name
        FROM scouter.spans s
        WHERE s.trace_id = decode(p_trace_id, 'hex')
          AND s.parent_span_id IS NULL
          AND (p_service_name IS NULL OR s.service_id = (SELECT service_id FROM service_filter))

        UNION ALL

        SELECT
            encode(s.trace_id, 'hex') as trace_id,
            encode(s.span_id, 'hex') as span_id,
            encode(s.parent_span_id, 'hex') as parent_span_id,
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
            s.output,
            s.service_name
        FROM scouter.spans s
        INNER JOIN span_tree st ON s.parent_span_id = st.span_id
        WHERE s.trace_id = decode(p_trace_id, 'hex')
          AND st.depth < 20
          AND (p_service_name IS NULL OR s.service_id = (SELECT service_id FROM service_filter))
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
        st.service_name,
        ROW_NUMBER() OVER (ORDER BY path) as span_order
    FROM span_tree st
    ORDER BY path;
$$;

CREATE OR REPLACE FUNCTION scouter.search_entities_by_tags(
    p_entity_type TEXT,
    p_tag_filters JSONB,
    p_match_all BOOLEAN DEFAULT TRUE
)
RETURNS TABLE (
    entity_id TEXT
)
LANGUAGE SQL
STABLE
AS $$
    WITH tag_filters AS (
        SELECT
            (filter->>'key')::TEXT as key,
            (filter->>'value')::TEXT as value
        FROM jsonb_array_elements(p_tag_filters) as filter
    ),
    filter_count AS (
        SELECT COUNT(*) as total FROM tag_filters
    )
    SELECT t.entity_id
    FROM scouter.tags t
    INNER JOIN tag_filters tf ON t.key = tf.key AND t.value = tf.value
    WHERE t.entity_type = p_entity_type
    GROUP BY t.entity_id
    HAVING
        CASE
            WHEN p_match_all THEN COUNT(DISTINCT t.key) = (SELECT total FROM filter_count)
            ELSE COUNT(DISTINCT t.key) > 0
        END;
$$;