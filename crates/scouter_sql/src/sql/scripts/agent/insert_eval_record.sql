WITH next_id AS (
    SELECT COALESCE(MAX(id), 0) + 1 AS id
    FROM scouter.agent_eval_record
    WHERE entity_id = $3
)
INSERT INTO scouter.agent_eval_record (
    id,
    uid,
    created_at,
    entity_id,
    context,
    record_id,
    session_id,
    trace_id,
    tags
)
SELECT
    next_id.id,
    $1, -- uid
    $2, -- created_at
    $3, -- entity_id
    $4, -- context
    $5, -- record_id
    $6, -- session_id
    $7, -- trace_id
    $8  -- tags
FROM next_id
ON CONFLICT DO NOTHING;