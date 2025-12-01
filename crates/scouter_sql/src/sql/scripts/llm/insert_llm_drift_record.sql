WITH next_id AS (
    SELECT COALESCE(MAX(id), 0) + 1 AS id
    FROM scouter.llm_drift_record
    WHERE entity_id = $2
)
INSERT INTO scouter.llm_drift_record (
    id, uid, created_at, entity_id, context, prompt
)
SELECT
    next_id.id,
    $1, -- uid
    $3, -- created_at
    $2, -- entity_id
    $4, -- context
    $5  -- prompt
FROM next_id
ON CONFLICT DO NOTHING;