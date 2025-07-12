INSERT INTO scouter.spc_drift (created_at, name, space, version, feature, value)
SELECT 
    created_at, name, space, version, feature, value
FROM UNNEST(
    $1::timestamptz[], 
    $2::text[], 
    $3::text[], 
    $4::text[], 
    $5::text[], 
    $6::double precision[]
) AS t(created_at, name, space, version, feature, value)
ON CONFLICT DO NOTHING;