INSERT INTO scouter.spans (
    span_id, 
    trace_id, 
    parent_span_id, 
    space, 
    name, 
    version, 
    scope,
    span_name, 
    span_kind, 
    start_time, 
    end_time, 
    duration_ms, 
    status_code, 
    status_message, 
    attributes, 
    events, 
    links
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
ON CONFLICT (span_id, trace_id, created_at) DO NOTHING