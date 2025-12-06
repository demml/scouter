WITH subquery1 AS (
    SELECT
        date_bin(($1 || ' minutes')::interval, created_at, TIMESTAMP '1970-01-01') as created_at,
        metric,
        value
    FROM scouter.llm_drift
    WHERE
        1=1
        AND created_at > CURRENT_TIMESTAMP - (interval '1 minute' * $2)
        AND entity_id = $3
    ),

subquery2 AS (
    SELECT
        created_at,
        metric,
        avg(value) as average,
        stddev(value) as standard_dev
    FROM subquery1
    GROUP BY
        created_at,
        metric
),

subquery3 AS (
    SELECT
        created_at,
        metric,
        jsonb_build_object(
            'avg', average,
            'lower_bound', average - coalesce(standard_dev,0),
            'upper_bound', average + coalesce(standard_dev,0)
        ) as stats
    FROM subquery2
)

SELECT
    metric,
    array_agg(created_at order by created_at desc) as created_at,
    array_agg(stats order by created_at desc) as stats
FROM subquery3
GROUP BY metric;