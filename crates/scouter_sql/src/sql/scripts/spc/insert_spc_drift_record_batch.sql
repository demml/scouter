INSERT INTO scouter.spc_drift (created_at, entity_id, feature, value)
SELECT
    created_at, entity_id, feature, value
FROM UNNEST(
    $1::timestamptz[],
    $2::integer[],
    $3::text[],
    $4::double precision[]
) AS t(created_at, entity_id, feature, value)
ON CONFLICT DO NOTHING;