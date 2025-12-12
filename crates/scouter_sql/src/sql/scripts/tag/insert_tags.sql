INSERT INTO scouter.tags (
    entity_type,
    entity_id,
    key,
    value,
    created_at,
    updated_at
)
SELECT
    entity_type,
    entity_id,
    key,
    value,
    NOW(),
    NOW()
FROM UNNEST(
    $1::text[], -- entity_type
    $2::text[], -- entity_id
    $3::text[], -- key
    $4::text[]  -- value
) AS t(
    entity_type,
    entity_id,
    key,
    value
)
ON CONFLICT (entity_type, entity_id, key)
DO UPDATE SET
    value = EXCLUDED.value,
    updated_at = NOW();