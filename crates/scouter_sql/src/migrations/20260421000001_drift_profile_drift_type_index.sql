CREATE INDEX IF NOT EXISTS idx_drift_profile_drift_type_active
    ON scouter.drift_profile (drift_type, active)
    WHERE active = true;
