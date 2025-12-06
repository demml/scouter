// this is a helper file for generating sql queries to retrieve binned data from datafusion.
use chrono::Datelike;
use chrono::Timelike;
use chrono::{DateTime, Utc};
pub fn get_binned_custom_metric_values_query(
    bin: &f64,
    start_time: &DateTime<Utc>,
    end_time: &DateTime<Utc>,
    entity_id: &i32,
) -> String {
    format!(
        r#"WITH subquery1 AS (
    SELECT
        date_bin(INTERVAL '{} minute', created_at, TIMESTAMP '1970-01-01') as created_at,
        metric,
        value
    FROM binned_custom_metric
    WHERE
        1=1
        AND created_at between TIMESTAMP '{}' AND TIMESTAMP '{}'
        AND entity_id = {}
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
        entity_id
    )
}

pub fn get_binned_llm_metric_values_query(
    bin: &f64,
    start_time: &DateTime<Utc>,
    end_time: &DateTime<Utc>,
    entity_id: &i32,
) -> String {
    format!(
        r#"WITH subquery1 AS (
    SELECT
        date_bin(INTERVAL '{} minute', created_at, TIMESTAMP '1970-01-01') as created_at,
        metric,
        value
    FROM binned_llm_metric
    WHERE
        1=1
        AND created_at between TIMESTAMP '{}' AND TIMESTAMP '{}'
        AND entity_id = {}
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
        entity_id
    )
}

pub fn get_binned_psi_drift_records_query(
    bin: &f64,
    start_time: &DateTime<Utc>,
    end_time: &DateTime<Utc>,
    entity_id: &i32,
) -> String {
    format!(
        r#"WITH feature_bin_total AS (
        SELECT
            date_bin('{} minutes', created_at, TIMESTAMP '1970-01-01') as created_at,
            entity_id,
            feature,
            bin_id,
            SUM(bin_count) AS bin_total_count
        FROM binned_psi
        WHERE
            1=1
            AND created_at between TIMESTAMP '{}' AND TIMESTAMP '{}'
            AND entity_id = {}
        GROUP BY 1, 2, 3, 4
    ),

    feature_total AS (
        SELECT
            date_bin('{} minutes', created_at, TIMESTAMP '1970-01-01') as created_at,
            entity_id,
            feature,
            cast(SUM(bin_count) as float) AS feature_total_count
        FROM binned_psi
        WHERE
            1=1
            AND created_at between TIMESTAMP '{}' AND TIMESTAMP '{}'
            AND entity_id = {}
        GROUP BY 1, 2, 3
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
            AND f.entity_id = b.entity_id
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
        entity_id,
        bin,
        start_time.to_rfc3339(),
        end_time.to_rfc3339(),
        entity_id,
    )
}

pub fn get_binned_spc_drift_records_query(
    bin: &f64,
    start_time: &DateTime<Utc>,
    end_time: &DateTime<Utc>,
    entity_id: &i32,
) -> String {
    let start_year = start_time.year();
    let start_month = start_time.month();
    let start_day = start_time.day();
    let start_hour = start_time.hour();

    let end_year = end_time.year();
    let end_month = end_time.month();
    let end_day = end_time.day();
    let end_hour = end_time.hour();

    format!(
        r#"WITH subquery1 AS (
        SELECT
            date_bin('{} minutes', created_at, TIMESTAMP '1970-01-01') as created_at,
            entity_id,
            feature,
            value
        FROM binned_spc
        WHERE
            -- Partition pruning predicates (inclusive)
            (year >= {start_year}) AND
            (year > {start_year} OR month >= {start_month}) AND
            (year > {start_year} OR month > {start_month} OR day >= {start_day}) AND
            (year > {start_year} OR month > {start_month} OR day > {start_day} OR hour >= {start_hour})
            AND
            (year <= {end_year}) AND
            (year < {end_year} OR month <= {end_month}) AND
            (year < {end_year} OR month < {end_month} OR day <= {end_day}) AND
            (year < {end_year} OR month < {end_month} OR day < {end_day} OR hour <= {end_hour})
            -- Regular filters (inclusive)
            AND created_at between TIMESTAMP '{}' AND TIMESTAMP '{}'
            AND entity_id = {}
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
        feature;"#,
        bin,
        start_time.to_rfc3339(),
        end_time.to_rfc3339(),
        entity_id
    )
}
