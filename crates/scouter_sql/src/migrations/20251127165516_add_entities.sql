-- =================================================================
-- SERVICE ENTITIES MIGRATION
-- Separates service tracking from entity tracking for traces/spans
-- =================================================================

-- =================================================================
-- STEP 1: DROP EXISTING DEPENDENCIES
-- =================================================================

DROP MATERIALIZED VIEW IF EXISTS scouter.trace_summary CASCADE;
DROP FUNCTION IF EXISTS scouter.get_trace_metrics CASCADE;
DROP FUNCTION IF EXISTS scouter.get_traces_paginated CASCADE;
DROP FUNCTION IF EXISTS scouter.get_trace_spans CASCADE;

-- =================================================================
-- STEP 2: CREATE SERVICE ENTITIES TABLE
-- =================================================================

CREATE TABLE IF NOT EXISTS scouter.service_entities (
    id SERIAL PRIMARY KEY,
    service_name TEXT UNIQUE NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);

CREATE INDEX idx_service_entities_name ON scouter.service_entities (service_name);

-- Helper function to get or create service_id
CREATE OR REPLACE FUNCTION scouter.get_or_create_service_id(p_service_name TEXT)
RETURNS INTEGER
LANGUAGE plpgsql
AS $$
DECLARE
    v_service_id INTEGER;
BEGIN
    IF p_service_name IS NULL THEN
        RETURN NULL;
    END IF;
    
    -- Try to get existing service_id
    SELECT id INTO v_service_id
    FROM scouter.service_entities
    WHERE service_name = p_service_name;
    
    -- If not found, insert and return new id
    IF v_service_id IS NULL THEN
        INSERT INTO scouter.service_entities (service_name)
        VALUES (p_service_name)
        ON CONFLICT (service_name) DO UPDATE
        SET updated_at = NOW()
        RETURNING id INTO v_service_id;
    END IF;
    
    RETURN v_service_id;
END;
$$;

-- =================================================================
-- STEP 3: SEED SERVICE ENTITIES FROM EXISTING TRACES
-- =================================================================

INSERT INTO scouter.service_entities (service_name)
SELECT DISTINCT service_name
FROM scouter.traces
WHERE service_name IS NOT NULL
ON CONFLICT (service_name) DO NOTHING;

-- =================================================================
-- STEP 4: ADD service_id COLUMNS TO TRACES AND SPANS
-- =================================================================

ALTER TABLE scouter.traces ADD COLUMN IF NOT EXISTS service_id INTEGER;
ALTER TABLE scouter.spans ADD COLUMN IF NOT EXISTS service_id INTEGER;

-- Backfill traces with service_id
UPDATE scouter.traces t
SET service_id = se.id
FROM scouter.service_entities se
WHERE t.service_name = se.service_name
  AND t.service_id IS NULL;

-- Backfill spans with service_id and service_name from parent trace
UPDATE scouter.spans s
SET 
    service_id = t.service_id,
    service_name = t.service_name
FROM scouter.traces t
WHERE s.trace_id = t.trace_id
  AND s.service_id IS NULL;

-- =================================================================
-- STEP 5: UPDATE TEMPLATE TABLES (for pg_partman)
-- =================================================================

ALTER TABLE scouter.template_scouter_traces ADD COLUMN IF NOT EXISTS service_id INTEGER;
ALTER TABLE scouter.template_scouter_spans ADD COLUMN IF NOT EXISTS service_id INTEGER;

-- =================================================================
-- STEP 6: CREATE OPTIMIZED INDEXES
-- =================================================================

-- Drop old indexes
DROP INDEX IF EXISTS scouter.idx_traces_service_lookup;
DROP INDEX IF EXISTS scouter.idx_spans_service_lookup;

-- Create optimized indexes using service_id
CREATE INDEX idx_traces_service_id_time ON scouter.traces (service_id, created_at DESC) 
    WHERE service_id IS NOT NULL;
CREATE INDEX idx_traces_service_id_status ON scouter.traces (service_id, status_code, created_at DESC) 
    WHERE service_id IS NOT NULL;

-- Keep service_name index for fallback during transition
CREATE INDEX idx_traces_service_name_lookup ON scouter.traces (service_name, created_at DESC) 
    WHERE service_name IS NOT NULL;

-- Span indexes
CREATE INDEX idx_spans_service_id_time ON scouter.spans (service_id, created_at DESC) 
    WHERE service_id IS NOT NULL;
CREATE INDEX idx_spans_service_id_errors ON scouter.spans (service_id, status_code, created_at DESC) 
    WHERE service_id IS NOT NULL AND status_code != 2;
CREATE INDEX idx_spans_service_name_lookup ON scouter.spans (service_name, created_at DESC) 
    WHERE service_name IS NOT NULL;

-- =================================================================
-- STEP 7: RECREATE TRACE_SUMMARY WITHOUT ENTITY_ID
-- =================================================================

CREATE MATERIALIZED VIEW scouter.trace_summary AS
SELECT
    t.trace_id,
    t.service_id,
    t.service_name,
    t.scope,
    span_stats.start_time,
    span_stats.end_time,
    EXTRACT(EPOCH FROM (span_stats.end_time - span_stats.start_time)) * 1000 AS duration_ms,
    t.status_code,
    t.status_message,
    t.span_count,
    t.created_at,
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
        MIN(start_time) as start_time,
        MAX(end_time) as end_time,
        AVG(duration_ms) as avg_span_duration,
        MAX(duration_ms) as max_span_duration
    FROM scouter.spans
    WHERE duration_ms IS NOT NULL
    AND created_at >= NOW() - INTERVAL '7 days'
    GROUP BY trace_id
) span_stats ON t.trace_id = span_stats.trace_id
WHERE t.created_at >= NOW() - INTERVAL '7 days';

-- Indexes for trace_summary
CREATE UNIQUE INDEX idx_trace_summary_unique ON scouter.trace_summary (trace_id, scope);
CREATE INDEX idx_trace_summary_service_id ON scouter.trace_summary (service_id, created_at DESC) 
    WHERE service_id IS NOT NULL;
CREATE INDEX idx_trace_summary_service_name ON scouter.trace_summary (service_name, created_at DESC) 
    WHERE service_name IS NOT NULL;

-- Schedule refresh
SELECT cron.schedule('refresh-trace-summary', '*/5 * * * *',
    $$REFRESH MATERIALIZED VIEW CONCURRENTLY scouter.trace_summary$$);

-- =================================================================
-- STEP 8: RECREATE QUERY FUNCTIONS
-- =================================================================

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
    )
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
            COUNT(*) FILTER (WHERE ts.has_errors = true) * 100.0 / NULLIF(COUNT(*), 0)
        ) as error_rate
    FROM scouter.trace_summary ts
    WHERE
        (p_service_name IS NULL OR ts.service_id IN (SELECT service_id FROM service_filter))
        AND ts.start_time >= p_start_time
        AND ts.start_time <= p_end_time
    GROUP BY bucket_start
    ORDER BY bucket_start DESC;
$$;

CREATE OR REPLACE FUNCTION scouter.get_traces_paginated(
    p_service_name TEXT DEFAULT NULL,
    p_has_errors BOOLEAN DEFAULT NULL,
    p_status_code INTEGER DEFAULT NULL,
    p_start_time TIMESTAMPTZ DEFAULT NOW() - INTERVAL '24 hours',
    p_end_time TIMESTAMPTZ DEFAULT NOW(),
    p_limit INTEGER DEFAULT 50,
    p_cursor_created_at TIMESTAMPTZ DEFAULT NULL,
    p_cursor_trace_id TEXT DEFAULT NULL,
    p_direction TEXT DEFAULT 'next'
)
RETURNS TABLE (
    trace_id TEXT,
    service_id INTEGER,
    service_name TEXT,
    scope TEXT,
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
    WITH service_filter AS (
        SELECT id as service_id
        FROM scouter.service_entities
        WHERE p_service_name IS NULL OR service_name = p_service_name
    )
    SELECT * FROM (
        SELECT
            ts.trace_id,
            ts.service_id,
            ts.service_name,
            ts.scope,
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
            (p_service_name IS NULL OR ts.service_id IN (SELECT service_id FROM service_filter))
            AND (p_has_errors IS NULL OR ts.has_errors = p_has_errors)
            AND (p_status_code IS NULL OR ts.status_code = p_status_code)
            AND ts.start_time >= p_start_time
            AND ts.start_time <= p_end_time
            AND (
                (p_direction = 'next' AND (
                    p_cursor_created_at IS NULL OR
                    ts.created_at < p_cursor_created_at OR
                    (ts.created_at = p_cursor_created_at AND ts.trace_id < p_cursor_trace_id)
                ))
                OR
                (p_direction = 'previous' AND (
                    p_cursor_created_at IS NULL OR
                    ts.created_at > p_cursor_created_at OR
                    (ts.created_at = p_cursor_created_at AND ts.trace_id > p_cursor_trace_id)
                ))
            )
        ORDER BY
            CASE WHEN p_direction = 'next' THEN ts.created_at END DESC,
            CASE WHEN p_direction = 'next' THEN ts.trace_id END DESC,
            CASE WHEN p_direction = 'previous' THEN ts.created_at END ASC,
            CASE WHEN p_direction = 'previous' THEN ts.trace_id END ASC
        LIMIT p_limit + 1
    ) sub
    ORDER BY
        CASE WHEN p_direction = 'next' THEN created_at END DESC,
        CASE WHEN p_direction = 'next' THEN trace_id END DESC,
        CASE WHEN p_direction = 'previous' THEN created_at END DESC,
        CASE WHEN p_direction = 'previous' THEN trace_id END DESC;
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
    span_order INTEGER
)
LANGUAGE SQL
STABLE
AS $$
   
    WITH service_filter AS (
        SELECT id as service_id
        FROM scouter.service_entities
        WHERE p_service_name IS NULL OR service_name = p_service_name
        LIMIT 1
    ),
    RECURSIVE span_tree AS (
      
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
          AND (p_service_name IS NULL OR s.service_id = (SELECT service_id FROM service_filter))

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
        WHERE s.trace_id = p_trace_id
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
        ROW_NUMBER() OVER (ORDER BY path) as span_order
    FROM span_tree st
    ORDER BY path;
$$;