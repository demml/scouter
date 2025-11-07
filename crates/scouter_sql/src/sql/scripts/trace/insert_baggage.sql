INSERT INTO scouter.trace_baggage (
    trace_id, 
    scope,
    key,
    value, 
    space, 
    name, 
    version
)
SELECT 
    trace_id, 
    scope,
    key,
    value, 
    space, 
    name, 
    version
FROM UNNEST(
    $1::text[], -- trace_id
    $2::text[], -- scope
    $3::text[], -- key
    $4::text[], -- value
    $5::text[], -- space
    $6::text[], -- name
    $7::text[]  -- version
) AS b(
    trace_id, 
    scope,
    key,
    value, 
    space, 
    name, 
    version
)
ON CONFLICT (trace_id, scope, key) DO NOTHING;