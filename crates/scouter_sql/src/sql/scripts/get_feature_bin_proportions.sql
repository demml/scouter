WITH feature_bin_total AS (SELECT name,
                                  repository,
                                  version,
                                  feature,
                                  bin_id,
                                  SUM(bin_count) AS bin_total_count
                           FROM scouter.observed_bin_count
                           WHERE name = $1
                             AND repository = $2
                             AND version = $3
                             AND created_at > $4::timestamp
                             AND feature = ANY($5)
                           GROUP BY 1, 2, 3, 4, 5),
    feature_total AS (SELECT name,
                             repository,
                             version,
                             feature,
                             SUM(bin_count) AS feature_total_count
                      FROM scouter.observed_bin_count
                      WHERE name = $1
                        AND repository = $2
                        AND version = $3
                        AND created_at > $4::timestamp
                        AND feature = ANY($5)
                      GROUP BY 1, 2, 3, 4),
    feature_bin_proportions AS (SELECT b.feature,
                                       f.feature_total_count,
                                       b.bin_id,
                                       b.bin_total_count::decimal / f.feature_total_count AS proportion
                                FROM feature_bin_total b
                                         JOIN
                                     feature_total f
                                ON f.feature = b.feature AND f.version = b.version AND
                                   f.repository = b.repository AND f.name = b.name)
SELECT feature,
       bin_id,
       proportion::FLOAT8
FROM feature_bin_proportions
WHERE feature_total_count > 1000