INSERT INTO scouter.trace_baggage (
    created_at,
    trace_id, 
    scope,
    key,
    value, 
    space, 
    name, 
    version
)
SELECT 
    created_at,
    trace_id, 
    scope,
    key,
    value, 
    space, 
    name, 
    version
FROM UNNEST(
    $1::timestamptz[],  -- created_at
    $2::text[], -- trace_id
    $3::text[], -- scope
    $4::text[], -- key
    $5::text[], -- value
    $6::text[], -- space
    $7::text[], -- name
    $8::text[] -- version
) AS b(
    created_at,
    trace_id, 
    scope,
    key,
    value, 
    space, 
    name, 
    version
)
ON CONFLICT (created_at, trace_id, scope, key) DO NOTHING;