WITH subquery1 AS (
    SELECT
        date_bin('$1 minutes', created_at, TIMESTAMP '1970-01-01') as created_at,
        name,
        space,
        feature,
        version,
        value
    FROM scouter.spc_drift
    WHERE 
        1=1
        AND created_at > timezone('utc', now()) - interval '$2 minute'
        AND name = $3
        AND space = $4
        AND version = $5
    ),

    subquery2 AS (
    SELECT
        created_at,
        name,
        space,
        feature,
        version,
        avg(value) as value
    FROM subquery1
    GROUP BY 
        created_at,
        name,
        space,
        feature,
        version
)

SELECT
feature,
array_agg(created_at ORDER BY created_at DESC) as created_at,
array_agg(value ORDER BY created_at DESC) as values
FROM subquery2
GROUP BY 
feature;