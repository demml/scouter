WITH feature_bin_total AS (
     SELECT
        date_bin(($1 || ' minutes')::interval, created_at, TIMESTAMP '1970-01-01') as created_at,
        entity_id,
        feature,
        bin_id,
        SUM(bin_count) AS bin_total_count
    FROM scouter.psi_drift
    WHERE
        1=1
        AND created_at >= $2  -- start_datetime
        AND created_at < $3   -- end_datetime
        AND entity_id = $4
    GROUP BY 1, 2, 3, 4
),

feature_total AS (
    SELECT
        date_bin(($1 || ' minutes')::interval, created_at, TIMESTAMP '1970-01-01') as created_at,
        entity_id,
        feature,
        SUM(bin_count) AS feature_total_count
    FROM scouter.psi_drift
    WHERE
        1=1
        AND created_at >= $2  -- start_datetime
        AND created_at < $3   -- end_datetime
        AND entity_id = $4
    GROUP BY 1, 2, 3
),

feature_bin_proportions AS (
    SELECT
        b.created_at,
        b.feature,
        f.feature_total_count,
        b.bin_id,
        b.bin_total_count::decimal / f.feature_total_count AS proportion
    FROM feature_bin_total b
    JOIN feature_total f
        ON f.feature = b.feature
        AND f.entity_id = b.entity_id
        AND f.created_at = b.created_at
),

overall_agg as (
    SELECT
        feature,
        jsonb_object_agg(bin_id, proportion::FLOAT8) as bins
    FROM feature_bin_proportions
    WHERE feature_total_count > 100
    GROUP BY feature
),

bin_agg as (
	SELECT
	    feature,
	    created_at,
	    jsonb_object_agg(
            bin_id, proportion::FLOAT8
        ) AS bin_proportions
	FROM feature_bin_proportions
	WHERE 1=1
	    AND feature_total_count > 100
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
    ON overall_agg.feature = feature_agg.feature
