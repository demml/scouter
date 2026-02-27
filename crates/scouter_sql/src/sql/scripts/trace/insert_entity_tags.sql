-- Batch insert trace-entity tags
INSERT INTO scouter.trace_entities (
    trace_id,
    entity_uid,
    tagged_at
)
WITH tag_data AS (
    SELECT
        trace_id,
        entity_uid,
        tagged_at
    FROM UNNEST(
        $1::bytea[],
        $2::bytea[],
        $3::timestamptz[]
    ) AS t(trace_id, entity_uid, tagged_at)
)
SELECT
    td.trace_id,
    td.entity_uid,
    td.tagged_at
FROM tag_data td
ON CONFLICT (entity_uid, tagged_at, trace_id) DO UPDATE
SET tagged_at = LEAST(EXCLUDED.tagged_at, scouter.trace_entities.tagged_at);
