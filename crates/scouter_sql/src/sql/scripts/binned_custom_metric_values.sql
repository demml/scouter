WITH subquery1 AS (
    SELECT
        date_bin('0.06 minutes', created_at, TIMESTAMP '1970-01-01') as created_at,
        metric,
        value
    FROM custom_metrics
    WHERE 
        1=1
        AND created_at > timezone('utc', now()) - interval '10 minute'
        AND name = 'test'
        AND repository = 'test'
        AND version ='test'
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
    array_agg(created_at) as created_at,
    array_agg(stats) as stats
FROM subquery3
GROUP BY metric;