INSERT INTO scouter.tags (
    created_at,
    entity_type,
    entity_id,
    key,
    value
)
SELECT
    created_at,
    entity_type,
    entity_id,
    key,
    value
FROM UNNEST(
    $1::timestamptz[],  -- created_at
    $2::text[], -- entity_type
    $3::text[], -- entity_id
    $4::text[], -- key
    $5::text[]  -- value
) AS b(
    created_at,
    entity_type,
    entity_id,
    key,
    value,
)
ON CONFLICT (created_at, entity_type, entity_id, key) DO NOTHING;