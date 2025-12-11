-- #################################################################
-- # REFACTORED MIGRATION SCRIPT: ENTITY & SERVICE ISOLATION
-- # FIXED: Handles tables missing 'drift_type' column dynamically
-- #################################################################

-- =================================================================
-- STEP 1: DROP ALL DEPENDENCIES (Functions & Materialized View)
-- =================================================================

DROP FUNCTION IF EXISTS scouter.get_trace_spans CASCADE;
DROP FUNCTION IF EXISTS scouter.get_traces_paginated CASCADE;
DROP FUNCTION IF EXISTS scouter.get_trace_metrics CASCADE;
DROP MATERIALIZED VIEW IF EXISTS scouter.trace_summary CASCADE;


-- drop trace tables
DROP TABLE IF EXISTS scouter.traces CASCADE;

DELETE FROM scouter.part_config
WHERE parent_table = 'scouter.traces';

DELETE FROM scouter.part_config_sub
WHERE sub_parent = 'scouter.traces';


SELECT cron.unschedule('refresh-trace-summary');


-- =================================================================
-- STEP 2: ENTITIES TABLE MIGRATION (For Drift/Metrics Data)
-- =================================================================

-- 2.1: Create the new central entities table
CREATE TABLE IF NOT EXISTS scouter.drift_entities (
    id SERIAL PRIMARY KEY,
    uid TEXT UNIQUE DEFAULT gen_random_uuid(),
    space TEXT NOT NULL,
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    drift_type TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE (space, name, version, drift_type)
);

CREATE INDEX IF NOT EXISTS idx_entities_lookup ON scouter.drift_entities (space, name, version, drift_type);

-- 2.2: Seed entities from existing drift_profile data
INSERT INTO scouter.drift_entities (space, name, version, drift_type)
SELECT DISTINCT space, name, version, drift_type FROM scouter.drift_profile
ON CONFLICT (space, name, version, drift_type) DO NOTHING;

-- 2.3: Update entities with existing uids from drift_profile
UPDATE scouter.drift_entities e
SET uid = dp.uid
FROM scouter.drift_profile dp
WHERE e.space = dp.space AND e.name = dp.name AND e.version = dp.version;

-- 2.4: Execute DDL and Backfill for tables that use the new 'entity_id' FK
DO $$
DECLARE
    entity_tables TEXT[] := ARRAY[
        'scouter.drift_profile',
        'scouter.observability_metric',
        'scouter.llm_drift',
        'scouter.llm_drift_record',
        'scouter.spc_drift',
        'scouter.drift_alert',
        'scouter.custom_drift',
        'scouter.psi_drift'
    ]::TEXT[];

    entity_template_tables TEXT[] := ARRAY[
        'scouter.template_scouter_custom_drift',
        'scouter.template_scouter_drift_alert',
        'scouter.template_scouter_llm_drift',
        'scouter.template_scouter_llm_drift_record',
        'scouter.template_scouter_observability_metric',
        'scouter.template_scouter_psi_drift',
        'scouter.template_scouter_spc_drift'
    ]::TEXT[];

    tbl TEXT;
    has_drift_type BOOLEAN;
    join_condition TEXT;
BEGIN
    -- Handle Entity Tables
    FOREACH tbl IN ARRAY entity_tables LOOP
        RAISE NOTICE 'Migrating Entity Table: %', tbl;

        -- 1. Add entity_id column
        EXECUTE format('ALTER TABLE %s ADD COLUMN IF NOT EXISTS entity_id INTEGER', tbl);

        -- 2. Check if table has drift_type column
        SELECT EXISTS (
            SELECT 1
            FROM information_schema.columns
            WHERE table_schema = split_part(tbl, '.', 1)
              AND table_name = split_part(tbl, '.', 2)
              AND column_name = 'drift_type'
        ) INTO has_drift_type;

        -- 3. Determine Join Condition based on column existence
        IF has_drift_type THEN
            join_condition := 'AND COALESCE(t.drift_type, '''') = COALESCE(e.drift_type, '''')';
        ELSE
            join_condition := 'AND e.drift_type IS NULL';
        END IF;

        -- 4. Backfill (Using Dynamic Join Condition)
        EXECUTE format($q$
            UPDATE %s t
            SET entity_id = e.id
            FROM scouter.drift_entities e
            WHERE t.space = e.space
              AND t.name = e.name
              AND t.version = e.version
              %s -- Injected Condition
              AND t.entity_id IS NULL
        $q$, tbl, join_condition);

        -- 5. Delete bad data and enforce NOT NULL
        EXECUTE format('DELETE FROM %s WHERE entity_id IS NULL', tbl);
        EXECUTE format('ALTER TABLE %s ALTER COLUMN entity_id SET NOT NULL', tbl);

        -- 6. Drop Old Columns
        BEGIN
            -- skip if scoute.drift_profile table (want to keep space/name for profile purposes)
            IF tbl != 'scouter.drift_profile' THEN
                EXECUTE format('ALTER TABLE %s DROP COLUMN IF EXISTS space, DROP COLUMN IF EXISTS name, DROP COLUMN IF EXISTS version CASCADE, DROP COLUMN IF EXISTS drift_type', tbl);
            END IF;
        EXCEPTION WHEN OTHERS THEN
            RAISE NOTICE 'Constraints dropping handled via CASCADE or manual cleanup for %', tbl;
        END;
    END LOOP;

    -- Handle Entity Template Tables
    FOREACH tbl IN ARRAY entity_template_tables LOOP
        RAISE NOTICE 'Updating Entity Template: %', tbl;
        EXECUTE format('ALTER TABLE %s ADD COLUMN IF NOT EXISTS entity_id INTEGER NOT NULL', tbl);
        EXECUTE format('ALTER TABLE %s DROP COLUMN IF EXISTS space, DROP COLUMN IF EXISTS name, DROP COLUMN IF EXISTS version CASCADE, DROP COLUMN IF EXISTS drift_type', tbl);
    END LOOP;

END $$;

-- 2.5: Add entity_uid to LLM tables
ALTER TABLE scouter.llm_drift RENAME COLUMN record_uid TO uid;


-- =================================================================
-- STEP 3: SERVICE ENTITIES MIGRATION (For Trace/Span Data)
-- =================================================================

-- 3.1: Create central service_entities table
CREATE TABLE IF NOT EXISTS scouter.service_entities (
    id SERIAL PRIMARY KEY,
    service_name TEXT UNIQUE NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_service_entities_name ON scouter.service_entities (service_name);

-- 3.2: Helper function to get or create service_id
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

    SELECT id INTO v_service_id
    FROM scouter.service_entities
    WHERE service_name = p_service_name;

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



-- 3.4: Add service_id column to traces and spans
ALTER TABLE scouter.spans ADD COLUMN IF NOT EXISTS resource_attributes JSONB;
ALTER TABLE scouter.spans ADD COLUMN IF NOT EXISTS service_id INTEGER;
ALTER TABLE scouter.spans ADD COLUMN IF NOT EXISTS service_name TEXT;

-- Drop template tables if they exist
DROP TABLE IF EXISTS scouter.template_scouter_traces CASCADE;

-- 3.7: Update Template Tables
ALTER TABLE scouter.template_scouter_spans ADD COLUMN IF NOT EXISTS service_id INTEGER;

-- 3.8: Drop old entity columns from traces/spans
DO $$
DECLARE
    trace_tables_with_old_cols TEXT[] := ARRAY[
        'scouter.spans',
        'scouter.template_scouter_spans'
    ]::TEXT[];
    tbl TEXT;
BEGIN
    FOREACH tbl IN ARRAY trace_tables_with_old_cols LOOP
        RAISE NOTICE 'Dropping old entity columns from Trace/Span Table: %', tbl;
        BEGIN
            EXECUTE format('ALTER TABLE %s DROP COLUMN IF EXISTS entity_id CASCADE', tbl);
            EXECUTE format('ALTER TABLE %s DROP COLUMN IF EXISTS space, DROP COLUMN IF EXISTS name, DROP COLUMN IF EXISTS version CASCADE', tbl);
        EXCEPTION WHEN OTHERS THEN
            RAISE NOTICE 'Constraints dropping handled via CASCADE or manual cleanup for %', tbl;
        END;
    END LOOP;
END $$;


-- =================================================================
-- STEP 4: CLEANUP OLD CONSTRAINTS & INDEXES
-- =================================================================

-- OBSERVABILITY METRIC
DROP INDEX IF EXISTS scouter.observability_metric_created_at_space_name_version_idx;
ALTER TABLE scouter.observability_metric DROP CONSTRAINT IF EXISTS observability_metric_created_at_name_space_version_key;
ALTER TABLE scouter.observability_metric ADD UNIQUE (created_at, entity_id);

-- LLM DRIFT
DROP INDEX IF EXISTS scouter.idx_llm_drift_created_at_space_name_version_metric;
ALTER TABLE scouter.llm_drift DROP CONSTRAINT IF EXISTS llm_drift_created_at_space_name_version_key;
ALTER TABLE scouter.llm_drift ADD UNIQUE (created_at, entity_id);

-- LLM DRIFT RECORD
DROP INDEX IF EXISTS scouter.idx_llm_drift_record_created_at_space_name_version;
DROP INDEX IF EXISTS scouter.idx_llm_drift_record_pagination;
ALTER TABLE scouter.llm_drift_record DROP CONSTRAINT IF EXISTS llm_drift_record_created_at_name_space_version_key;

-- SPC DRIFT
DROP INDEX IF EXISTS scouter.idx_spc_drift_created_at_space_name_version_feature;
ALTER TABLE scouter.spc_drift DROP CONSTRAINT IF EXISTS spc_drift_created_at_name_space_feature_value_version_key;
ALTER TABLE scouter.spc_drift ADD UNIQUE (created_at, entity_id, feature, value);

-- CUSTOM DRIFT
DROP INDEX IF EXISTS scouter.idx_custom_drift_created_at_space_name_version_metric;
ALTER TABLE scouter.custom_drift DROP CONSTRAINT IF EXISTS custom_drift_created_at_name_space_version_key;
ALTER TABLE scouter.custom_drift ADD UNIQUE (created_at, entity_id);

-- PSI DRIFT
DROP INDEX IF EXISTS scouter.idx_psi_drift_created_at_space_name_version_feature;
ALTER TABLE scouter.psi_drift DROP CONSTRAINT IF EXISTS psi_drift_created_at_name_space_version_feature_bin_id_key;
ALTER TABLE scouter.psi_drift ADD UNIQUE (created_at, entity_id, feature, bin_id);

-- DRIFT ALERT
DROP INDEX IF EXISTS scouter.idx_drift_alert_created_at_space_name_version;
ALTER TABLE scouter.drift_alert DROP CONSTRAINT IF EXISTS drift_alert_created_at_name_space_version_key;
ALTER TABLE scouter.drift_alert ADD UNIQUE (entity_id, created_at);

-- TRACES
DROP INDEX IF EXISTS scouter.idx_traces_entity_lookup;
DROP INDEX IF EXISTS scouter.idx_traces_created_at;
DROP INDEX IF EXISTS scouter.idx_traces_duration_analysis;
DROP INDEX IF EXISTS scouter.idx_traces_time_covering;
DROP INDEX IF EXISTS scouter.idx_traces_entity_covering;

-- SPANS
DROP INDEX IF EXISTS scouter.idx_spans_entity_lookup;
DROP INDEX IF EXISTS scouter.idx_spans_error_analysis;


-- =================================================================
-- STEP 5: RECREATE INDEXES
-- =================================================================

-- SPANS
CREATE INDEX idx_spans_trace_lookup ON scouter.spans(trace_id, start_time);
CREATE INDEX idx_spans_service_time ON scouter.spans(service_id, start_time DESC);
CREATE INDEX idx_spans_root ON scouter.spans(trace_id) WHERE parent_span_id IS NULL;
CREATE INDEX IF NOT EXISTS idx_spans_service_id_time ON scouter.spans (service_id, created_at DESC)
    WHERE service_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_spans_service_id_errors ON scouter.spans (service_id, status_code, created_at DESC)
    WHERE service_id IS NOT NULL AND status_code != 2;
CREATE INDEX IF NOT EXISTS idx_spans_parent_tree ON scouter.spans (parent_span_id, trace_id)
    WHERE parent_span_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_spans_service_name_fallback ON scouter.spans (service_name, created_at DESC)
    WHERE service_name IS NOT NULL;

-- DRIFT/METRICS LOOKUPS
CREATE INDEX IF NOT EXISTS idx_llm_drift_lookup ON scouter.llm_drift (created_at, entity_id);
CREATE INDEX IF NOT EXISTS idx_llm_drift_record_lookup ON scouter.llm_drift_record (entity_id, created_at);
CREATE INDEX IF NOT EXISTS idx_llm_drift_record_pagination ON scouter.llm_drift_record (entity_id, id DESC);
CREATE INDEX IF NOT EXISTS idx_observability_lookup ON scouter.observability_metric (created_at, entity_id);

-- =================================================================
-- STEP 7: RECREATE QUERY FUNCTIONS
-- =================================================================

-- 7.1: get_trace_metrics
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
            -- Trace timing is min/max of all spans
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
        (COUNT(*) FILTER (WHERE bm.status_code = 2) * 100.0 / NULLIF(COUNT(*), 0))::FLOAT8 as error_rate
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
            -- Root span determines trace ownership
            (SELECT service_name FROM scouter.spans
             WHERE trace_id = s.trace_id
             AND parent_span_id IS NULL
             LIMIT 1) as service_name,
            -- Get scope from root span
            (SELECT scope FROM scouter.spans
             WHERE trace_id = s.trace_id
             AND parent_span_id IS NULL
             LIMIT 1) as scope,
            -- Get root operation name
            (SELECT span_name FROM scouter.spans
             WHERE trace_id = s.trace_id
             AND parent_span_id IS NULL
             LIMIT 1) as root_operation,
            -- Get resource attributes from root span
            (SELECT resource_attributes FROM scouter.spans
             WHERE trace_id = s.trace_id
             AND parent_span_id IS NULL
             LIMIT 1) as resource_attributes,
            -- Trace timing is min/max of all spans
            MIN(s.start_time) as start_time,
            MAX(s.end_time) as end_time,
            EXTRACT(EPOCH FROM (MAX(s.end_time) - MIN(s.start_time))) * 1000 as duration_ms,
            -- Overall trace status (worst span status)
            MAX(s.status_code) as status_code,
            -- Status message from any error span
            (SELECT status_message FROM scouter.spans
             WHERE trace_id = s.trace_id
             AND status_code = 2
             LIMIT 1) as status_message,
            -- Statistics
            COUNT(*) as span_count,
            COUNT(*) FILTER (WHERE s.status_code = 2) as error_count
        FROM scouter.spans s
        WHERE
            s.start_time >= p_start_time
            AND s.start_time <= p_end_time
            -- Service filter applied to root spans only
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
            s.input, s.output
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
            s.input, s.output
        FROM scouter.spans s
        INNER JOIN span_tree st ON s.parent_span_id = st.span_id
        WHERE s.trace_id = p_trace_id
          AND st.depth < 20
          AND (p_service_name IS NULL OR s.service_id = (SELECT service_id FROM service_filter))
    )
    SELECT
        st.trace_id, st.span_id, st.parent_span_id, st.span_name, st.span_kind, st.start_time, st.end_time, st.duration_ms,
        st.status_code, st.status_message, st.attributes, st.events, st.links,
        st.depth, st.path, st.root_span_id, st.input, st.output,
        ROW_NUMBER() OVER (ORDER BY path) as span_order
    FROM span_tree st
    ORDER BY path;
$$;