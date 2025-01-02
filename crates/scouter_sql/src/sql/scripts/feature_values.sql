
WITH subquery AS (SELECT
created_at,
feature,
value
FROM drift
WHERE
    created_at > $1::timestamp
    AND name = $2
    AND repository = $3
    AND version = $4
)

SELECT
    feature,
    array_agg(created_at ORDER BY created_at DESC) as created_at,
    array_agg(value ORDER BY created_at DESC) as values
FROM subquery
GROUP BY 
    feature;