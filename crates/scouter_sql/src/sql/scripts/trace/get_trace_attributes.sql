SELECT
    trace_id,
    process_attributes
FROM scouter.traces
WHERE trace_id = $1;