SELECT
    bucket_start as "bucket_start!",
    trace_count as "trace_count!",
    avg_duration_ms as "avg_duration_ms!",
    p50_duration_ms as "p50_duration_ms",
    p95_duration_ms as "p95_duration_ms",
    p99_duration_ms as "p99_duration_ms",
    error_rate as "error_rate!"
FROM scouter.get_trace_metrics(
    $1, -- p_space: TEXT DEFAULT NULL
    $2, -- p_name: TEXT DEFAULT NULL
    $3, -- p_version: TEXT DEFAULT NULL
    $4, -- p_start_time: TIMESTAMPTZ DEFAULT NOW() - INTERVAL '1 hour'
    $5, -- p_end_time: TIMESTAMPTZ DEFAULT NOW()
    $6::INTERVAL -- p_bucket_interval: INTERVAL DEFAULT '5 minutes'
)