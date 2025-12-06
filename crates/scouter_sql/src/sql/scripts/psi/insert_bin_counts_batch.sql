INSERT INTO scouter.psi_drift (created_at, entity_id, feature, bin_id, bin_count)
SELECT
    created_at, entity_id, feature, bin_id, bin_count
FROM UNNEST(
    $1::timestamptz[],
    $2::integer[],
    $3::text[],
    $4::integer[],
    $5::integer[]
) AS t(created_at, entity_id, feature, bin_id, bin_count)
ON CONFLICT DO NOTHING;