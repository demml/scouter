INSERT INTO scouter.trace_baggage (
    trace_id, 
    key,
    value, 
    space, 
    name, 
    version
    ) VALUES ($1, $2, $3, $4, $5, $6)
    ON CONFLICT (trace_id, key, created_at) DO NOTHING