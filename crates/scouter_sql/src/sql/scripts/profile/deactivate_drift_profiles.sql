UPDATE scouter.drift_profile
SET active = false,
    updated_at = timezone('utc', now())
WHERE space = $1
  and name = $2
  and version != $3;
  and ($4 IS NULL OR drift_type = $4);