INSERT INTO scouter.trace_baggage (
    created_at,
    trace_id,
    scope,
    key,
    value
)
SELECT * FROM UNNEST(
    $1::timestamptz[],  -- created_at
    $2::bytea[], -- trace_id
    $3::text[], -- scope
    $4::text[], -- key
    $5::text[] -- value
)
ON CONFLICT (created_at, trace_id, scope, key) DO NOTHING;