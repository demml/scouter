// this is a helper file for generating sql queries to retrieve binned data from datafusion.
use chrono::{DateTime, Utc};

pub fn get_binned_custom_metric_values_query(
    bin: &i32,
    start_time: &DateTime<Utc>,
    end_time: &DateTime<Utc>,
    space: &str,
    name: &str,
    version: &str,
) -> String {
    format!(
        r#"WITH subquery1 AS (
    SELECT
        date_bin(INTERVAL '{} minute', created_at, TIMESTAMP '1970-01-01') as created_at,
        metric,
        value
    FROM custom_metric
    WHERE 
        1=1
        AND created_at between TIMESTAMP '{}' AND TIMESTAMP '{}'
        AND space = '{}'
        AND name = '{}'
        AND version = '{}'
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
        struct(
            average as avg,
            average - COALESCE(standard_dev, 0) as lower_bound,
            average + COALESCE(standard_dev, 0) as upper_bound
        ) as stats
    FROM subquery2
)

SELECT 
    metric,
    array_agg(created_at) as created_at,
    array_agg(stats) as stats
FROM subquery3
GROUP BY metric;"#,
        bin,
        start_time.to_rfc3339(),
        end_time.to_rfc3339(),
        name,
        space,
        version
    )
}
