INSERT INTO scouter.agent_eval_record (
    uid,
    created_at,
    entity_id,
    context,
    record_id,
    session_id,
    trace_id,
    tags
)
VALUES (
    $1, -- uid
    $2, -- created_at
    $3, -- entity_id
    $4, -- context
    $5, -- record_id
    $6, -- session_id
    $7, -- trace_id
    $8  -- tags
)
ON CONFLICT ON CONSTRAINT idx_agent_eval_record_entity_trace DO NOTHING;