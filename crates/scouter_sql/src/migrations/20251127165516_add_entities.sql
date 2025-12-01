-- Add migration script here
-- =================================================================
-- STEP 1: DROP DEPENDENCIES
-- =================================================================

DROP MATERIALIZED VIEW IF EXISTS scouter.trace_summary;
DROP FUNCTION IF EXISTS scouter.get_trace_metrics;
DROP FUNCTION IF EXISTS scouter.get_traces_paginated;
DROP FUNCTION IF EXISTS scouter.get_trace_spans;

-- =================================================================
-- STEP 2: CREATE AND SEED ENTITIES TABLE
-- =================================================================

CREATE TABLE IF NOT EXISTS scouter.entities (
    id SERIAL PRIMARY KEY,
    uid TEXT UNIQUE DEFAULT gen_random_uuid(),
    space TEXT NOT NULL,
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    drift_type TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE (space, name, version, drift_type)
);

CREATE INDEX idx_entities_lookup ON scouter.entities (space, name, version, drift_type);

INSERT INTO scouter.entities (space, name, version, drift_type)
SELECT DISTINCT space, name, version, drift_type FROM scouter.drift_profile
ON CONFLICT (space, name, version, drift_type) DO NOTHING;


UPDATE scouter.entities e
SET uid = dp.uid
FROM scouter.drift_profile dp
WHERE e.space = dp.space AND e.name = dp.name AND e.version = dp.version;

DO $$
DECLARE
    -- Standard tables
    target_tables TEXT[] := ARRAY[
        'scouter.drift_profile',
        'scouter.observability_metric',
        'scouter.llm_drift',
        'scouter.llm_drift_record',
        'scouter.spc_drift',
        'scouter.drift_alert',
        'scouter.custom_drift',
        'scouter.psi_drift',
        'scouter.traces',
        'scouter.spans'
    ]::TEXT[];

    -- Template tables (for pg_partman future partitions)
    template_tables TEXT[] := ARRAY[
        'scouter.template_scouter_custom_drift',
        'scouter.template_scouter_drift_alert',
        'scouter.template_scouter_llm_drift',
        'scouter.template_scouter_llm_drift_record',
        'scouter.template_scouter_observability_metric',
        'scouter.template_scouter_psi_drift',
        'scouter.template_scouter_spans',
        'scouter.template_scouter_spc_drift',
        'scouter.template_scouter_traces'
    ]::TEXT[];

    tbl TEXT;
BEGIN
    -- 1. Handle Active Tables (Add Column -> Backfill -> Drop Old)
    FOREACH tbl IN ARRAY target_tables LOOP
        RAISE NOTICE 'Migrating Active Table: %', tbl;

        -- Add entity_id
        EXECUTE format('ALTER TABLE %s ADD COLUMN IF NOT EXISTS entity_id INTEGER', tbl);

        -- Backfill (Heavy Operation)
        -- Joins on the composite key to find the new ID
        EXECUTE format('
            UPDATE %s t
            SET entity_id = e.id
            FROM scouter.entities e
            WHERE t.space = e.space
              AND t.name = e.name
              AND t.version = e.version
              AND t.entity_id IS NULL
        ', tbl);

        -- Enforce Constraints (Traces/Spans can be NULL, others NOT NULL)
        IF tbl = 'scouter.traces' OR tbl = 'scouter.spans' THEN
            -- Allowed to be NULL
        ELSE
            -- Delete bad data or enforcement will fail
            EXECUTE format('DELETE FROM %s WHERE entity_id IS NULL', tbl);
            EXECUTE format('ALTER TABLE %s ALTER COLUMN entity_id SET NOT NULL', tbl);
        END IF;

        -- Drop Old Columns
        BEGIN
            EXECUTE format('ALTER TABLE %s DROP COLUMN IF EXISTS space, DROP COLUMN IF EXISTS name, DROP COLUMN IF EXISTS version CASCADE', tbl);
        EXCEPTION WHEN OTHERS THEN
            RAISE NOTICE 'Constraints dropping handled via CASCADE or manual cleanup for %', tbl;
        END;
    END LOOP;

    -- 2. Handle Partman Templates (Add Column -> Drop Old)
    FOREACH tbl IN ARRAY template_tables LOOP
        RAISE NOTICE 'Updating Template: %', tbl;

        EXECUTE format('ALTER TABLE %s ADD COLUMN IF NOT EXISTS entity_id INTEGER', tbl);

        IF tbl NOT LIKE '%traces' AND tbl NOT LIKE '%spans' THEN
             EXECUTE format('ALTER TABLE %s ALTER COLUMN entity_id SET NOT NULL', tbl);
        END IF;

        EXECUTE format('ALTER TABLE %s DROP COLUMN IF EXISTS space, DROP COLUMN IF EXISTS name, DROP COLUMN IF EXISTS version CASCADE', tbl);
    END LOOP;

END $$;

-- =================================================================
-- STEP 4: CLEANUP OLD COLUMNS & CONSTRAINTS
-- =================================================================

-- add entity_uid to llm_drift_record
ALTER TABLE scouter.llm_drift_record ADD COLUMN IF NOT EXISTS entity_uid TEXT;
-- rename record_uid to entity_uid in llm_drift_record
ALTER TABLE scouter.llm_drift RENAME COLUMN record_uid TO uid;
ALTER TABLE scouter.llm_drift ADD COLUMN IF NOT EXISTS entity_uid TEXT;

-- 1. Observability Metric
DROP INDEX IF EXISTS scouter.observability_metric_created_at_space_name_version_idx;
ALTER TABLE scouter.observability_metric DROP CONSTRAINT IF EXISTS observability_metric_created_at_name_space_version_key;
ALTER TABLE scouter.observability_metric ADD UNIQUE (created_at, entity_id);

-- 2. LLM Drift
DROP INDEX IF EXISTS scouter.idx_llm_drift_created_at_space_name_version_metric;
ALTER TABLE scouter.llm_drift DROP CONSTRAINT IF EXISTS llm_drift_created_at_space_name_version_key;

-- 3. LLM Drift Record
DROP INDEX IF EXISTS scouter.idx_llm_drift_record_created_at_space_name_version;
DROP INDEX IF EXISTS scouter.idx_llm_drift_record_pagination;
ALTER TABLE scouter.llm_drift_record DROP CONSTRAINT IF EXISTS llm_drift_record_created_at_name_space_version_key;

-- 4. SPC Drift
DROP INDEX IF EXISTS scouter.idx_spc_drift_created_at_space_name_version_feature;
ALTER TABLE scouter.spc_drift DROP CONSTRAINT IF EXISTS spc_drift_created_at_name_space_feature_value_version_key;
ALTER TABLE scouter.spc_drift ADD UNIQUE (created_at, entity_id, feature, value);

-- 6. Custom Drift
DROP INDEX IF EXISTS scouter.idx_custom_drift_created_at_space_name_version_metric;
ALTER TABLE scouter.custom_drift DROP CONSTRAINT IF EXISTS custom_drift_created_at_name_space_version_key;
ALTER TABLE scouter.custom_drift ADD UNIQUE (created_at, entity_id);

-- 7. PSI Drift
DROP INDEX IF EXISTS scouter.idx_psi_drift_created_at_space_name_version_feature;
ALTER TABLE scouter.psi_drift DROP CONSTRAINT IF EXISTS psi_drift_created_at_name_space_version_feature_bin_id_key;
ALTER TABLE scouter.psi_drift ADD UNIQUE (created_at, entity_id, feature, bin_id);

-- 8. Drift Alert
DROP INDEX IF EXISTS scouter.idx_drift_alert_created_at_space_name_version;
ALTER TABLE scouter.drift_alert DROP CONSTRAINT IF EXISTS drift_alert_created_at_name_space_version_key;
ALTER TABLE scouter.drift_alert ADD UNIQUE (entity_id, created_at);

-- 9. Traces
-- Drop constraints
ALTER TABLE scouter.traces DROP CONSTRAINT IF EXISTS traces_created_at_trace_id_space_name_version_key;
-- Drop indexes
DROP INDEX IF EXISTS scouter.idx_traces_entity_lookup;
DROP INDEX IF EXISTS scouter.idx_traces_created_at;
DROP INDEX IF EXISTS scouter.idx_traces_duration_analysis;
DROP INDEX IF EXISTS scouter.idx_traces_time_covering;
DROP INDEX IF EXISTS scouter.idx_traces_entity_covering;

-- 10. Spans
DROP INDEX IF EXISTS scouter.idx_spans_entity_lookup;
DROP INDEX IF EXISTS scouter.idx_spans_error_analysis;

-- =================================================================
-- STEP 5: RECREATE INDEXES (Optimized for Entity ID)
-- =================================================================

-- TRACES
CREATE INDEX idx_traces_entity_lookup ON scouter.traces (entity_id, created_at DESC) WHERE entity_id IS NOT NULL;
CREATE INDEX idx_traces_created_at ON scouter.traces (created_at DESC);
CREATE INDEX idx_traces_duration_analysis ON scouter.traces (entity_id, duration_ms DESC) WHERE duration_ms IS NOT NULL;
CREATE INDEX idx_traces_entity_covering ON scouter.traces (entity_id, created_at DESC)
INCLUDE (trace_id, start_time, end_time, duration_ms, status_code, span_count);

-- SPANS
CREATE INDEX idx_spans_entity_lookup ON scouter.spans (entity_id, created_at DESC) WHERE entity_id IS NOT NULL;
CREATE INDEX idx_spans_error_analysis ON scouter.spans (entity_id, status_code, created_at DESC) WHERE status_code = 2;
CREATE INDEX idx_spans_parent_tree ON scouter.spans (parent_span_id, trace_id)
WHERE parent_span_id IS NOT NULL;

-- LLM DRIFT
CREATE INDEX idx_llm_drift_lookup ON scouter.llm_drift (created_at, entity_id, metric);
ALTER TABLE scouter.llm_drift ADD UNIQUE (created_at, entity_id);

-- LLM RECORD
CREATE INDEX idx_llm_drift_record_lookup ON scouter.llm_drift_record (entity_id, created_at);
CREATE INDEX idx_llm_drift_record_pagination ON scouter.llm_drift_record (entity_id, id DESC);

-- OBSERVABILITY
CREATE INDEX idx_observability_lookup ON scouter.observability_metric (created_at, entity_id);


CREATE MATERIALIZED VIEW IF NOT EXISTS scouter.trace_summary AS
SELECT
    t.trace_id,
    t.entity_id,
    e.space,
    e.name,
    e.version,
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
-- LEFT JOIN because entity_id can be NULL
LEFT JOIN scouter.entities e ON t.entity_id = e.id
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
        MAX(duration_ms) as max_span_duration
    FROM scouter.spans
    WHERE duration_ms IS NOT NULL
    AND created_at >= NOW() - INTERVAL '7 days'
    GROUP BY trace_id
) span_stats ON t.trace_id = span_stats.trace_id
WHERE t.created_at >= NOW() - INTERVAL '7 days';

CREATE UNIQUE INDEX IF NOT EXISTS idx_trace_summary_unique
ON scouter.trace_summary (trace_id, scope);

CREATE INDEX IF NOT EXISTS idx_trace_summary_entity
ON scouter.trace_summary (entity_id, created_at DESC);

-- Refresh Schedule
SELECT cron.schedule('refresh-trace-summary', '*/5 * * * *',
    $$REFRESH MATERIALIZED VIEW CONCURRENTLY scouter.trace_summary$$);


CREATE OR REPLACE FUNCTION scouter.get_trace_metrics(
    p_entity_id INTEGER DEFAULT NULL,
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
        (p_entity_id IS NULL OR ts.entity_id = p_entity_id)
        AND ts.start_time >= p_start_time
        AND ts.start_time <= p_end_time
    GROUP BY bucket_start
    ORDER BY bucket_start DESC;
$$;


CREATE OR REPLACE FUNCTION scouter.get_traces_paginated(
    p_entity_id INTEGER DEFAULT NULL,
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
    entity_id INTEGER,
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
    SELECT * FROM (
        SELECT
            ts.trace_id,
            ts.entity_id,
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
            (p_entity_id IS NULL OR ts.entity_id = p_entity_id)
            AND (p_service_name IS NULL OR ts.service_name = p_service_name)
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
    p_entity_id INTEGER DEFAULT NULL
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
    WITH RECURSIVE span_tree AS (
        -- Anchor: Find root spans (no parent)
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
          AND (p_entity_id IS NULL OR s.entity_id = p_entity_id)

        UNION ALL

        -- Recursive: Walk down the span tree
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
          AND (p_entity_id IS NULL OR s.entity_id = p_entity_id)
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