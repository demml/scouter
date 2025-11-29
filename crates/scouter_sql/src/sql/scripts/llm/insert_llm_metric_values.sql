INSERT INTO scouter.llm_drift (created_at, record_uid, entity_id, metric, value)
SELECT
    created_at, record_uid, entity_id, metric, value
FROM UNNEST(
    $1::timestamptz[],
    $2::text[],
    $3::integer[],
    $6::text[],
    $7::double precision[]
) AS t(created_at, record_uid, entity_id, metric, value)
ON CONFLICT DO NOTHING;