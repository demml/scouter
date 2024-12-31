UPDATE scouter.drift_profile
SET previous_run = next_run,
    next_run     = $1,
    updated_at   = timezone('utc', now())
WHERE name = $2
  and repository = $3
  and version = $4;