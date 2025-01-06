SELECT name, repository, version, profile, drift_type, previous_run, schedule
FROM drift_profile
WHERE active
  AND next_run < CURRENT_TIMESTAMP
LIMIT 1 FOR UPDATE SKIP LOCKED;