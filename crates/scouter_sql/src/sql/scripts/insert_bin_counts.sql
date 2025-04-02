INSERT INTO scouter.observed_bin_count (created_at, name, space, version, feature, bin_id, bin_count)
VALUES ($1, $2, $3, $4, $5, $6, $7)
    ON CONFLICT DO NOTHING;
