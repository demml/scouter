INSERT INTO scouter.psi_drift (created_at, name, space, version, feature, bin_id, bin_count)
SELECT 
    created_at, name, space, version, feature, bin_id, bin_count
FROM UNNEST(
    $1::timestamptz[], 
    $2::text[], 
    $3::text[], 
    $4::text[], 
    $5::text[], 
    $6::integer[], 
    $7::integer[]
) AS t(created_at, name, space, version, feature, bin_id, bin_count)
ON CONFLICT DO NOTHING;