WITH next_id AS (
    SELECT COALESCE(MAX(id), 0) + 1 AS id
    FROM scouter.llm_drift_record
    WHERE space = $2 AND name = $3 AND version = $4
)
INSERT INTO scouter.llm_drift_record (
    id, created_at, space, name, version, context, prompt
)
SELECT next_id.id, $1, $2, $3, $4, $5, $6
FROM next_id
ON CONFLICT DO NOTHING;