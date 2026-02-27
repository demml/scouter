-- Add migration script here
CREATE OR REPLACE FUNCTION scouter.get_trace_metrics(
    p_service_name TEXT DEFAULT NULL,
    p_start_time TIMESTAMPTZ DEFAULT NOW() - INTERVAL '1 hour',
    p_end_time TIMESTAMPTZ DEFAULT NOW(),
    p_bucket_interval INTERVAL DEFAULT '5 minutes',
    p_attribute_filters JSONB DEFAULT NULL,
    p_match_all_attributes BOOLEAN DEFAULT FALSE,
    p_entity_uid TEXT DEFAULT NULL
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
    with matching_traces AS (
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
    entity_filter AS (
        SELECT te.trace_id
        FROM scouter.trace_entities te
        INNER JOIN scouter.drift_entities de ON te.entity_id = de.id
        WHERE de.uid = p_entity_uid
          AND te.tagged_at >= COALESCE(p_start_time, NOW() - INTERVAL '24 hours')
          AND te.tagged_at <= COALESCE(p_end_time, NOW())
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
            -- Apply entity filter if entity_uid is provided
            AND (p_entity_uid IS NULL OR s.trace_id IN (SELECT trace_id FROM entity_filter))
        GROUP BY s.trace_id
        -- Apply service filter to root spans after grouping if service_name is specified
        HAVING p_service_name IS NULL
            OR EXISTS (
                SELECT 1 FROM scouter.spans root
                WHERE root.trace_id = s.trace_id
                AND root.parent_span_id IS NULL
                AND root.service_name = p_service_name
            )
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
