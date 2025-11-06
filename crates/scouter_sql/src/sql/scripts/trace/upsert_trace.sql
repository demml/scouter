INSERT INTO scouter.traces (
    trace_id, 
    space, 
    name, 
    version, 
    drift_type, 
    service_name,
    trace_state,
    start_time, 
    end_time, 
    duration_ms, 
    status,
    root_span_id
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
ON CONFLICT (trace_id, service_name) DO UPDATE SET
    end_time = EXCLUDED.end_time,
    duration_ms = EXCLUDED.duration_ms,
    status = EXCLUDED.status,
    span_count = scouter.traces.span_count + 1,
    trace_state = EXCLUDED.trace_state,
    updated_at = NOW();
