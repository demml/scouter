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
        space,
        name,
        version
    )
}

pub fn get_binned_psi_drift_records_query(
    bin: &i32,
    start_time: &DateTime<Utc>,
    end_time: &DateTime<Utc>,
    space: &str,
    name: &str,
    version: &str,
) -> String {
    format!(
        r#"WITH feature_bin_total AS (
        SELECT 
            date_bin('{} minutes', created_at, TIMESTAMP '1970-01-01') as created_at,
            name,
            space,
            version,
            feature,
            bin_id,
            SUM(bin_count) AS bin_total_count
        FROM psi_metrics
        WHERE 
            1=1
            AND created_at between TIMESTAMP '{}' AND TIMESTAMP '{}'
            AND space = '{}'
            AND name = '{}'
            AND version = '{}'
        GROUP BY 1, 2, 3, 4, 5, 6
    ),

    feature_total AS (
        SELECT 
            date_bin('{} minutes', created_at, TIMESTAMP '1970-01-01') as created_at,
            name,
            space,
            version,
            feature,
            cast(SUM(bin_count) as float) AS feature_total_count
        FROM psi_metrics
        WHERE 
            1=1
            AND created_at between TIMESTAMP '{}' AND TIMESTAMP '{}'
            AND space = '{}'
            AND name = '{}'
            AND version = '{}'
        GROUP BY 1, 2, 3, 4, 5
    ),

    feature_bin_proportions AS (
        SELECT 
            b.created_at,
            b.feature,
            f.feature_total_count,
            b.bin_id,
            cast(b.bin_total_count as float) / f.feature_total_count AS proportion
        FROM feature_bin_total b
        JOIN feature_total f
            ON f.feature = b.feature 
            AND f.version = b.version 
            AND f.space = b.space
            AND f.name = b.name
            AND f.created_at = b.created_at
    ),

    overall_agg as (
        SELECT 
            feature,
            struct(
                array_agg(bin_id) as bin_id, 
                array_agg(proportion) as proportion
            ) as bins
        FROM feature_bin_proportions
        WHERE feature_total_count > 1
        GROUP BY feature
    ),

    bin_agg as (
        SELECT 
            feature,
            created_at,
            struct(
                array_agg(bin_id) as bin_id, 
                array_agg(proportion) as proportion
            ) AS bin_proportions
        FROM feature_bin_proportions
        WHERE feature_total_count > 1
        GROUP BY 
            feature, 
            created_at
    ),

    feature_agg as (
    select
    feature,
    array_agg(created_at order by created_at desc) as created_at,
    array_agg(bin_proportions order by created_at desc) as bin_proportions
    FROM bin_agg
    WHERE 1=1
    GROUP BY feature
    )

    SELECT 
        feature_agg.feature,
        created_at,
        bin_proportions,
        bins as overall_proportions
    FROM feature_agg
    JOIN overall_agg
        ON overall_agg.feature = feature_agg.feature;"#,
        bin,
        start_time.to_rfc3339(),
        end_time.to_rfc3339(),
        space,
        name,
        version,
        bin,
        start_time.to_rfc3339(),
        end_time.to_rfc3339(),
        space,
        name,
        version
    )
}
