-- Add migration script here
ALTER TABLE scouter.drift_profile
  ADD COLUMN major INTEGER NOT NULL DEFAULT 0,
  ADD COLUMN minor INTEGER NOT NULL DEFAULT 0,
  ADD COLUMN patch INTEGER NOT NULL DEFAULT 0,
  ADD COLUMN pre_tag TEXT,
  ADD COLUMN build_tag TEXT;

