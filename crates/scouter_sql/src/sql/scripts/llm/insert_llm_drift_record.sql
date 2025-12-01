WITH next_id AS (
    SELECT COALESCE(MAX(id), 0) + 1 AS id
    FROM scouter.llm_drift_record
    WHERE entity_id = $2
)
INSERT INTO scouter.llm_drift_record (
    id, uid, created_at, entity_id, entity_uid, context, prompt
)
SELECT next_id.id, $1, $2, $3, $4, $5, $6
FROM next_id
ON CONFLICT DO NOTHING;