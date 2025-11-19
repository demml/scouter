SELECT
    created_at,
    trace_id,
    scope,
    key,
    value
FROM scouter.trace_baggage
WHERE trace_id = $1;