WITH subquery AS (
        SELECT
        created_at,
        feature,
        value
    FROM scouter.spc_drift
    where
        1=1
        AND created_at > $1
        AND entity_id = $2
        AND feature = ANY($3)
),
aggregated AS (
    SELECT
        feature,
        array_agg(created_at ORDER BY created_at DESC) as created_at,
        array_agg(value ORDER BY created_at DESC) as values
    FROM subquery
    GROUP BY
        feature
),
min_length AS (
    SELECT
        MIN(array_length(created_at, 1)) as min_len
    FROM aggregated
)

SELECT
    feature,
    (created_at)[:(SELECT min_len FROM min_length)] as created_at,
    (values)[:(SELECT min_len FROM min_length)] as values
FROM aggregated
GROUP BY
    feature,
    created_at,
    values;