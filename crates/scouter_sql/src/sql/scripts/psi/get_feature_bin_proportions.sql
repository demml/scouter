WITH profile_bin_counts AS (SELECT name,
                                   space,
                                   version,
                                   feature_key                                 AS feature,
                                   JSONB_ARRAY_LENGTH(feature_value -> 'bins') AS bin_count
                            FROM scouter.drift_profile,
                                 JSONB_EACH(profile -> 'features') AS features(feature_key, feature_value)
                            WHERE name = $1
                              AND space = $2
                              AND version = $3),
     feature_bin_total AS (SELECT name,
                                  space,
                                  version,
                                  feature,
                                  bin_id,
                                  SUM(bin_count) AS bin_total_count
                           FROM scouter.psi_drift
                           WHERE 1 = 1
                             AND created_at > $4
                             AND space = $2
                             AND name = $1
                             AND version = $3
                             AND feature = ANY ($5)
                           GROUP BY 1, 2, 3, 4, 5)
        ,
     feature_total AS (SELECT name,
                              space,
                              version,
                              feature,
                              SUM(bin_count) AS feature_total_count
                       FROM scouter.psi_drift
                       WHERE 1 = 1
                         AND created_at > $4
                         AND space = $2
                         AND name = $1
                         AND version = $3
                         AND feature = ANY ($5)
                       GROUP BY 1, 2, 3, 4),
     filtered_feature_total AS (SELECT ft.*,
                                       (10 * pbc.bin_count)
                                FROM feature_total ft
                                         JOIN profile_bin_counts pbc
                                              ON ft.name = pbc.name
                                                  AND ft.space = pbc.space
                                                  AND ft.version = pbc.version
                                                  AND ft.feature = pbc.feature
                                /*
                                    PSI Minimum Sample Size: 10 * number_of_bins
                                    Based on Yurdakul (2018) "Statistical Properties of Population Stability Index"
                                    - Prevents unreliable PSI calculations from small samples
                                    - Attempts to Avoid PSI = infinity when bins have 0 observations
                                    - Ensures chi-square approximation validity
                                    Citation: https://scholarworks.wmich.edu/dissertations/3208
                                */
                                WHERE ft.feature_total_count > (10 * pbc.bin_count)),
     feature_bin_proportions AS (SELECT b.feature,
                                        f.feature_total_count,
                                        b.bin_id,
                                        b.bin_total_count::decimal / f.feature_total_count AS proportion
                                 FROM feature_bin_total b
                                          JOIN filtered_feature_total f
                                               ON f.feature = b.feature
                                                   AND f.version = b.version
                                                   AND f.space = b.space
                                                   AND f.name = b.name)
SELECT feature,
       feature_total_count                          AS sample_size,
       JSONB_OBJECT_AGG(bin_id, proportion::FLOAT8) AS bins
FROM feature_bin_proportions
GROUP BY feature, sample_size

