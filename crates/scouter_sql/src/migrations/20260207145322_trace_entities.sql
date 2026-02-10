-- Dedicated junction table for trace-entity many-to-many relationships
-- Partitioned by tagged_at for efficient time-based pruning

CREATE TABLE IF NOT EXISTS scouter.trace_entities (
    trace_id BYTEA NOT NULL CHECK (octet_length(trace_id) = 16),
    entity_id INTEGER NOT NULL REFERENCES scouter.drift_entities(id) ON DELETE CASCADE,
    tagged_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (entity_id, tagged_at, trace_id)
) PARTITION BY RANGE (tagged_at);

-- Create partitioned table using pg_partman (1-day partitions)
SELECT scouter.create_parent(
    'scouter.trace_entities',
    'tagged_at',
    '1 day'
);

-- Configure partition maintenance: premake 7 days ahead, retain 30 days
UPDATE scouter.part_config
    SET premake = 7,
    retention = '30 days',
    retention_keep_table = FALSE
WHERE parent_table = 'scouter.trace_entities';

-- Hot path index: lookup trace_ids by entity_id
-- This index is automatically created on each partition by pg_partman
-- Ordering by tagged_at DESC supports time-bounded queries efficiently
CREATE INDEX idx_trace_entities_entity_lookup
ON scouter.trace_entities (entity_id, tagged_at DESC)
INCLUDE (trace_id);

-- Secondary index: reverse lookup (trace -> entities), useful for some analytics
CREATE INDEX idx_trace_entities_trace_lookup
ON scouter.trace_entities (trace_id, tagged_at DESC)
INCLUDE (entity_id);

-- Comments for context
COMMENT ON TABLE scouter.trace_entities IS
'Many-to-many junction table mapping traces to entities (prompts, data sources, services, etc.). Optimized for high-volume queries.';


-- Function: Get traces for a single entity (hot path)
CREATE OR REPLACE FUNCTION scouter.get_traces_by_entity(
    p_entity_uid TEXT,
    p_start_time TIMESTAMPTZ DEFAULT NOW() - INTERVAL '24 hours',
    p_end_time TIMESTAMPTZ DEFAULT NOW(),
    p_limit INTEGER DEFAULT 50,
    p_cursor_start_time TIMESTAMPTZ DEFAULT NULL,
    p_cursor_trace_id BYTEA DEFAULT NULL
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
    resource_attributes JSONB
)
LANGUAGE SQL
STABLE
AS $$

    WITH entity_lookup AS (
        SELECT id as entity_id
        FROM scouter.drift_entities
        WHERE uid = p_entity_uid
        LIMIT 1
    ),
    entity_traces AS (
        SELECT DISTINCT te.trace_id
        FROM scouter.trace_entities te
        WHERE te.entity_id = (SELECT entity_id FROM entity_lookup)
          AND te.tagged_at >= p_start_time
          AND te.tagged_at <= p_end_time
    )
    SELECT
        encode(t.trace_id, 'hex') as trace_id,
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
        t.error_count > 0 AS has_errors,
        t.error_count,
        t.resource_attributes
    FROM scouter.traces t
    WHERE t.trace_id IN (SELECT trace_id FROM entity_traces)
      AND t.bucket_time >= p_start_time
      AND t.bucket_time <= p_end_time
      AND (
          p_cursor_start_time IS NULL OR
          (t.start_time, t.trace_id) < (p_cursor_start_time, p_cursor_trace_id)
      )
    ORDER BY t.start_time DESC, t.trace_id
    LIMIT p_limit;
$$;


-- Function: Get traces matching ALL entities (service view with multiple filters)
CREATE OR REPLACE FUNCTION scouter.get_traces_by_multiple_entities(
    p_entity_uids TEXT[],
    p_match_all BOOLEAN DEFAULT TRUE,
    p_start_time TIMESTAMPTZ DEFAULT NOW() - INTERVAL '24 hours',
    p_end_time TIMESTAMPTZ DEFAULT NOW(),
    p_limit INTEGER DEFAULT 50
)
RETURNS TABLE (
    trace_id TEXT,
    service_name TEXT,
    root_operation TEXT,
    start_time TIMESTAMPTZ,
    duration_ms BIGINT,
    error_count BIGINT,
    matched_entities TEXT[]
)
LANGUAGE SQL
STABLE
AS $$

    WITH entity_lookup AS (
        SELECT id as entity_id, uid as entity_uid
        FROM scouter.drift_entities
        WHERE uid = ANY(p_entity_uids)
    ),
    entity_traces AS (
        SELECT
            te.trace_id,
            array_agg(DISTINCT e.entity_uid) as matched_entities,
            COUNT(DISTINCT te.entity_id) as match_count
        FROM scouter.trace_entities te
        INNER JOIN entity_lookup e ON te.entity_id = e.entity_id
        WHERE te.tagged_at >= p_start_time
          AND te.tagged_at <= p_end_time
        GROUP BY te.trace_id
        HAVING
            CASE
                WHEN p_match_all THEN
                    COUNT(DISTINCT te.entity_id) = array_length(p_entity_uids, 1)
                ELSE
                    COUNT(DISTINCT te.entity_id) > 0
            END
    )
    SELECT
        encode(t.trace_id, 'hex') as trace_id,
        t.service_name,
        t.root_operation,
        t.start_time,
        t.duration_ms,
        t.error_count,
        et.matched_entities
    FROM scouter.traces t
    INNER JOIN entity_traces et ON t.trace_id = et.trace_id
    WHERE t.bucket_time >= p_start_time
      AND t.bucket_time <= p_end_time
    ORDER BY t.start_time DESC
    LIMIT p_limit;
$$;


-- Function: Get trace metrics grouped by entity (for analytics)
CREATE OR REPLACE FUNCTION scouter.get_entity_trace_metrics(
    p_entity_uids TEXT[],
    p_start_time TIMESTAMPTZ DEFAULT NOW() - INTERVAL '24 hours',
    p_end_time TIMESTAMPTZ DEFAULT NOW()
)
RETURNS TABLE (
    entity_uid TEXT,
    entity_space TEXT,
    entity_name TEXT,
    entity_version TEXT,
    drift_type TEXT,
    trace_count BIGINT,
    avg_duration_ms FLOAT8,
    error_rate FLOAT8,
    p95_duration_ms FLOAT8
)
LANGUAGE SQL
STABLE
AS $$
    -- Join to drift_entities to get entity metadata
    WITH entity_lookup AS (
        SELECT id, uid, space, name, version, drift_type
        FROM scouter.drift_entities
        WHERE uid = ANY(p_entity_uids)
    )
    SELECT
        e.uid as entity_uid,
        e.space as entity_space,
        e.name as entity_name,
        e.version as entity_version,
        e.drift_type,
        COUNT(DISTINCT t.trace_id) as trace_count,
        AVG(t.duration_ms)::FLOAT8 as avg_duration_ms,
        (SUM(CASE WHEN t.error_count > 0 THEN 1 ELSE 0 END)::FLOAT8 /
         NULLIF(COUNT(*), 0) * 100)::FLOAT8 as error_rate,
        PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY t.duration_ms)::FLOAT8 as p95_duration_ms
    FROM scouter.trace_entities te
    INNER JOIN entity_lookup e ON te.entity_id = e.id
    INNER JOIN scouter.traces t ON te.trace_id = t.trace_id
    WHERE te.tagged_at >= p_start_time
      AND te.tagged_at <= p_end_time
      AND t.bucket_time >= p_start_time
      AND t.bucket_time <= p_end_time
    GROUP BY e.uid, e.space, e.name, e.version, e.drift_type
    ORDER BY trace_count DESC;
$$;
