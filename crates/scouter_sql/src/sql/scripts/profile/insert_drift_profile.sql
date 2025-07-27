INSERT INTO scouter.drift_profile (space, name, major, minor, patch, pre_tag, build_tag, version, scouter_version, profile, drift_type, active, schedule, next_run, previous_run)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
ON CONFLICT DO NOTHING;