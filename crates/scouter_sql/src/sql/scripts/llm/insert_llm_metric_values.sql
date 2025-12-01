INSERT INTO scouter.llm_drift (created_at, uid, entity_id, entity_uid, metric, value)
SELECT
    created_at, uid, entity_id, entity_uid, metric, value
FROM UNNEST(
    $1::timestamptz[],
    $2::text[],
    $3::integer[],
    $4::text[],
    $5::text[],
    $6::double precision[]
) AS t(created_at, uid, entity_id, entity_uid, metric, value)
ON CONFLICT DO NOTHING;