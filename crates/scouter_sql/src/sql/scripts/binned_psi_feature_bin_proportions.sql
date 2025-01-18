WITH feature_bin_total AS (
     SELECT 
        date_bin('$1 minutes', created_at, TIMESTAMP '1970-01-01') as created_at,
        name,
        repository,
        version,
        feature,
        bin_id,
        SUM(bin_count) AS bin_total_count
    FROM observed_bin_count
    WHERE 
        1=1
        AND created_at > timezone('utc', now()) - interval '$2 minute'
        AND name = $3
        AND repository = $4
        AND version = $5
    GROUP BY 1, 2, 3, 4, 5, 6
),
feature_total AS (
    SELECT 
        date_bin('$1 minutes', created_at, TIMESTAMP '1970-01-01') as created_at,
        name,
        repository,
        version,
        feature,
        SUM(bin_count) AS feature_total_count
    FROM observed_bin_count
    WHERE 
        1=1
        AND created_at > timezone('utc', now()) - interval '$2 minute'
        AND name = $3
        AND repository = $4
        AND version = $5
    GROUP BY 1, 2, 3, 4, 5
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
        AND f.version = b.version 
        AND f.repository = b.repository 
        AND f.name = b.name
        AND f.created_at = b.created_at
),


bin_agg as (
	SELECT 
	    feature,
	    created_at,
	    jsonb_object_agg(
            bin_id, proportion::FLOAT8
        ) AS bin_proportions
    ) as bin_proportions
	FROM feature_bin_proportions
	WHERE 1=1
	    AND feature_total_count > 100
	GROUP BY 
		feature, 
		created_at
)

select
 feature,
 array_agg(created_at order by created_at desc) as created_at,
 array_agg(bin_proportions order by created_at desc) as bin_proportions
FROM bin_agg
WHERE 1=1
GROUP BY feature