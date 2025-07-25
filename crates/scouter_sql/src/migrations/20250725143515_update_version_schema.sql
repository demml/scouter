-- Add migration script here
ALTER TABLE scouter.drift_profile
  ADD COLUMN major INTEGER,
  ADD COLUMN minor INTEGER,
  ADD COLUMN patch INTEGER,
  ADD COLUMN pre_tag TEXT,
  ADD COLUMN build_tag TEXT;

-- 2. Backfill new columns from version string
UPDATE scouter.drift_profile
SET
  major = (regexp_matches(version, '^(\d+)\.(\d+)\.(\d+)'))[1]::INTEGER,
  minor = (regexp_matches(version, '^(\d+)\.(\d+)\.(\d+)'))[2]::INTEGER,
  patch = (regexp_matches(version, '^(\d+)\.(\d+)\.(\d+)'))[3]::INTEGER,

-- 3. Set NOT NULL and DEFAULT constraints for new columns
ALTER TABLE scouter.drift_profile
  ALTER COLUMN major SET NOT NULL,
  ALTER COLUMN minor SET NOT NULL,
  ALTER COLUMN patch SET NOT NULL,