-- Search entities by tag key-value pairs
-- Returns entity_ids matching the provided tag filters
SELECT entity_id
FROM scouter.search_entities_by_tags(
    $1,  -- p_entity_type (e.g., 'trace', 'span')
    $2,  -- p_tag_filters (JSONB array: [{"key": "environment", "value": "production"}, ...])
    $3   -- p_match_all (boolean: true for AND logic, false for OR logic)
);