UPDATE scouter.drift_profile
SET active = false,
    updated_at = CURRENT_TIMESTAMP
WHERE space = $1
  and name = $2
  and version != $3
  and ($4 IS NULL OR drift_type = $4);