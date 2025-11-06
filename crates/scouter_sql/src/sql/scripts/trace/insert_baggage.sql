INSERT INTO scouter.trace_baggage (
    trace_id, 
    scope,
    key,
    value, 
    space, 
    name, 
    version
    ) VALUES ($1, $2, $3, $4, $5, $6, $7)
    ON CONFLICT (trace_id, scope, key) DO NOTHING