INSERT INTO scouter.custom_drift (created_at, entity_id, metric, value)
SELECT
    created_at, entity_id, metric, value
FROM UNNEST(
    $1::timestamptz[],
    $2::int[],
    $3::text[],
    $4::double precision[]
) AS t(created_at, entity_id, metric, value)
ON CONFLICT DO NOTHING;