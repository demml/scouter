WITH next_id AS (
    SELECT COALESCE(MAX(id), 0) + 1 AS id
    FROM scouter.llm_event_record
    WHERE space = $2 AND name = $3 AND version = $4
)
INSERT INTO scouter.llm_event_record (
    id, 
    created_at, 
    space, 
    name, 
    version, 
    prompt,
    inputs,
    outputs,
    ground_truth,
    metadata,
    entity_type,
    root_id,
    event_id,
    event_name,
    parent_event_name,
    duration_ms
)
SELECT 
    next_id.id, 
    $1, 
    $2, 
    $3, 
    $4, 
    $5, 
    $6,
    $7,
    $8,
    $9,
    $10,
    $11,
    $12,
    $13,
    $14,
    $15
FROM next_id
ON CONFLICT DO NOTHING;