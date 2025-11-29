INSERT INTO scouter.psi_drift (created_at, entity_id, feature, bin_id, bin_count)
VALUES ($1, $2, $3, $4, $5)
    ON CONFLICT DO NOTHING;
