SELECT
created_at,
entity_type,
entity_id,
key,
value
FROM scouter.tags
WHERE entity_type = $1
  AND entity_id = $2