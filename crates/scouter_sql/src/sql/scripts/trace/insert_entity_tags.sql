-- Batch insert trace-entity tags
-- Resolves entity_uids to entity_ids in a single CTE for efficiency
INSERT INTO scouter.trace_entities (
    trace_id,
    entity_id,
    tagged_at
)
WITH entity_lookup AS (
    -- Resolve all unique entity_uids to entity_ids in one query
    SELECT DISTINCT ON (uid) id, uid
    FROM scouter.drift_entities
    WHERE uid = ANY($2::text[])
),
tag_data AS (
    SELECT
        trace_id,        -- Already bytea from UNNEST, no decode needed
        entity_uid,
        tagged_at
    FROM UNNEST(
        $1::bytea[],      -- trace_id as bytea[]
        $2::text[],       -- entity_uid
        $3::timestamptz[] -- tagged_at
    ) AS t(trace_id, entity_uid, tagged_at)
)
SELECT
    td.trace_id,
    el.id as entity_id,
    td.tagged_at
FROM tag_data td
INNER JOIN entity_lookup el ON td.entity_uid = el.uid
ON CONFLICT (entity_id, tagged_at, trace_id) DO UPDATE
SET tagged_at = LEAST(EXCLUDED.tagged_at, scouter.trace_entities.tagged_at);
