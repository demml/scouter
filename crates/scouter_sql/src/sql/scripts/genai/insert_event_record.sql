WITH next_id AS (
    SELECT COALESCE(MAX(id), 0) + 1 AS id
    FROM scouter.genai_event_record
    WHERE entity_id = $3
)
INSERT INTO scouter.genai_event_record (
    id, uid, created_at, entity_id, context, prompt
)
SELECT
    next_id.id,
    $1, -- uid
    $2, -- created_at
    $3, -- entity_id
    $4, -- context
    $5  -- prompt
FROM next_id
ON CONFLICT DO NOTHING;