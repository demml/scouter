INSERT INTO scouter.genai_drift (created_at, uid, entity_id, metric, value)
SELECT
    created_at, uid, entity_id, metric, value
FROM UNNEST(
    $1::timestamptz[], -- created_at
    $2::text[],        -- uid
    $3::integer[],     -- entity_id
    $4::text[],        -- metric
    $5::double precision[] -- value
) AS t(
    created_at, uid, entity_id, metric, value
)
ON CONFLICT DO NOTHING;