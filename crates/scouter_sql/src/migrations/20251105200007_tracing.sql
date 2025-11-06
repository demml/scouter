-- Tables for traces, spans, and baggage in Scouter tracing module
CREATE TABLE IF NOT EXISTS scouter.traces (
    trace_id TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    
    -- Scouter entity correlation fields
    space TEXT NOT NULL,
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    drift_type TEXT,
    
    -- Trace metadata
    service_name TEXT NOT NULL,
    trace_state TEXT,
    
    -- Timing information
    start_time TIMESTAMPTZ NOT NULL,
    end_time TIMESTAMPTZ,
    duration_ms BIGINT,
    
    status TEXT NOT NULL DEFAULT 'ok',
    root_span_id TEXT,
    span_count INTEGER DEFAULT 0,

    archived BOOLEAN DEFAULT FALSE,
    
    PRIMARY KEY (trace_id, service_name),
    UNIQUE (created_at, trace_id, space, name, version)
) PARTITION BY RANGE (created_at);

-- Spans table - stores individual span data
CREATE TABLE IF NOT EXISTS scouter.spans (
    span_id TEXT NOT NULL,
    trace_id TEXT NOT NULL,
    parent_span_id TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    
    -- Scouter entity correlation (denormalized for performance)
    space TEXT NOT NULL,
    name TEXT NOT NULL, 
    version TEXT NOT NULL,
    drift_type TEXT,
    
    -- Span core data
    service_name TEXT NOT NULL,
    operation_name TEXT NOT NULL,
    span_kind TEXT NOT NULL DEFAULT 'internal', -- server, client, producer, consumer, internal
    
    -- Timing
    start_time TIMESTAMPTZ NOT NULL,
    end_time TIMESTAMPTZ,
    duration_ms BIGINT,
    
    -- Status and error handling
    status_code TEXT NOT NULL DEFAULT 'ok',
    status_message TEXT,
    
    -- Attributes and events
    attributes JSONB DEFAULT '{}',
    events JSONB DEFAULT '[]',
    links JSONB DEFAULT '[]',
    instrumentation_scope JSONB DEFAULT '{}',
    
    -- Cleanup
    archived BOOLEAN DEFAULT FALSE,
    
    PRIMARY KEY (span_id, trace_id, created_at),
    UNIQUE (created_at, trace_id, span_id, space, name, version)
) PARTITION BY RANGE (created_at);

-- Baggage table - stores baggage key-value pairs
CREATE TABLE IF NOT EXISTS scouter.trace_baggage (
    trace_id TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    
    -- Baggage data
    service_name TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    metadata JSONB DEFAULT '{}',
    
    space TEXT NOT NULL,
    name TEXT NOT NULL,
    version TEXT NOT NULL,

    PRIMARY KEY (trace_id, service_name, key),
    UNIQUE (created_at, trace_id, key, space, name, version)
) PARTITION BY RANGE (created_at);

-- Performance indexes for traces table
CREATE INDEX idx_traces_entity_lookup 
ON scouter.traces (space, name, version, created_at DESC);

CREATE INDEX idx_traces_service_time 
ON scouter.traces (service_name, start_time DESC);

CREATE INDEX idx_traces_status_time 
ON scouter.traces (status, created_at DESC) 
WHERE status != 'ok';

CREATE INDEX idx_traces_duration_analysis 
ON scouter.traces (space, name, version, duration_ms DESC) 
WHERE duration_ms IS NOT NULL;

CREATE INDEX idx_spans_trace_hierarchy 
ON scouter.spans (trace_id, parent_span_id, start_time);

CREATE INDEX idx_spans_entity_lookup 
ON scouter.spans (space, name, version, created_at DESC);

CREATE INDEX idx_spans_operation_performance 
ON scouter.spans (operation_name, span_kind, duration_ms DESC) 
WHERE duration_ms IS NOT NULL;

CREATE INDEX idx_spans_error_analysis 
ON scouter.spans (space, name, version, status_code, created_at DESC) 
WHERE status_code != 'ok';

CREATE INDEX idx_spans_attributes 
ON scouter.spans USING GIN (attributes);

CREATE INDEX idx_spans_events 
ON scouter.spans USING GIN (events);

CREATE INDEX idx_baggage_entity_lookup 
ON scouter.trace_baggage (space, name, version, created_at DESC);

CREATE INDEX idx_baggage_key_lookup 
ON scouter.trace_baggage (key, created_at DESC);

-- Partitioning configuration
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

-- Configure 30-day retention for all tracing tables
UPDATE scouter.part_config SET retention = '30 days' 
WHERE parent_table IN ('scouter.traces', 'scouter.spans', 'scouter.trace_baggage');

-- Materialized views for common analytical queries
CREATE MATERIALIZED VIEW scouter.trace_analytics_daily AS
SELECT 
    DATE_TRUNC('day', created_at) as day,
    space,
    name, 
    version,
    service_name,
    COUNT(*) as trace_count,
    AVG(duration_ms) as avg_duration_ms,
    PERCENTILE_CONT(0.50) WITHIN GROUP (ORDER BY duration_ms) as p50_duration_ms,
    PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY duration_ms) as p95_duration_ms,
    PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY duration_ms) as p99_duration_ms,
    COUNT(*) FILTER (WHERE status = 'error') as error_count,
    COUNT(*) FILTER (WHERE status = 'error') * 100.0 / COUNT(*) as error_rate_percent
FROM scouter.traces 
WHERE created_at >= CURRENT_DATE - INTERVAL '30 days'
    AND duration_ms IS NOT NULL
GROUP BY 1, 2, 3, 4, 5;

CREATE UNIQUE INDEX idx_trace_analytics_daily_unique 
ON scouter.trace_analytics_daily (day, space, name, version, service_name);

-- Refresh materialized view daily
SELECT cron.schedule('refresh-trace-analytics', '0 1 * * *', 
    $$REFRESH MATERIALIZED VIEW CONCURRENTLY scouter.trace_analytics_daily$$);

-- Additional view for span performance analysis
CREATE MATERIALIZED VIEW scouter.span_performance_daily AS
SELECT 
    DATE_TRUNC('day', created_at) as day,
    space,
    name,
    version, 
    operation_name,
    span_kind,
    COUNT(*) as span_count,
    AVG(duration_ms) as avg_duration_ms,
    PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY duration_ms) as p95_duration_ms,
    COUNT(*) FILTER (WHERE status_code = 'error') as error_count
FROM scouter.spans
WHERE created_at >= CURRENT_DATE - INTERVAL '30 days'
    AND duration_ms IS NOT NULL
GROUP BY 1, 2, 3, 4, 5, 6;

CREATE UNIQUE INDEX idx_span_performance_daily_unique 
ON scouter.span_performance_daily (day, space, name, version, operation_name, span_kind);

SELECT cron.schedule('refresh-span-performance', '0 1 * * *',
    $$REFRESH MATERIALIZED VIEW CONCURRENTLY scouter.span_performance_daily$$);