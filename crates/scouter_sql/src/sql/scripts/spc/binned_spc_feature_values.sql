WITH subquery1 AS (
    SELECT
        date_bin(($1 || ' minutes')::interval, created_at, TIMESTAMP '1970-01-01') as created_at,
        entity_id,
        feature,
        value
    FROM scouter.spc_drift
    WHERE
        1=1
        AND created_at > CURRENT_TIMESTAMP - (interval '1 minute' * $2)
        AND entity_id = $3
    ),

    subquery2 AS (
    SELECT
        created_at,
        entity_id,
        feature,
        avg(value) as value
    FROM subquery1
    GROUP BY
        created_at,
        entity_id,
        feature
)

SELECT
feature,
array_agg(created_at ORDER BY created_at DESC) as created_at,
array_agg(value ORDER BY created_at DESC) as values
FROM subquery2
GROUP BY
feature;