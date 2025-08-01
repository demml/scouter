INSERT INTO scouter.llm_drift (created_at, record_uid, space, name, version, metric, value)
SELECT 
    created_at, record_uid, space, name, version, metric, value
FROM UNNEST(
    $1::timestamptz[], 
    $2::text[], 
    $3::text[], 
    $4::text[], 
    $5::text[], 
    $6::text[], 
    $7::double precision[]
) AS t(created_at, record_uid, space, name, version, metric, value)
ON CONFLICT DO NOTHING;