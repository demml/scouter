-- Migrations

CREATE EXTENSION if not exists pg_partman SCHEMA scouter;
CREATE EXTENSION if not exists pg_cron;

CREATE TABLE IF NOT exists scouter.spc_drift (
  created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
  name text,
  space text,
  feature text,
  value double precision,
  version text,
  updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
  archived boolean default false,
  UNIQUE (created_at,name,space,feature,value,version)
)
PARTITION BY RANGE (created_at);

CREATE INDEX ON scouter.spc_drift (name, space, version, created_at);

SELECT scouter.create_parent(
    'scouter.spc_drift',
    'created_at',
    '1 day'
);

UPDATE scouter.part_config SET retention = '60 days' WHERE parent_table = 'scouter.spc_drift';

-- Create table for service drift configuration
CREATE table IF NOT exists scouter.drift_profile (
  uid TEXT PRIMARY KEY DEFAULT gen_random_uuid(),
  created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
  updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
  name text,
  space text,
  version text,
  drift_type text,
  profile jsonb,
  active boolean default true,
  schedule  text,
  next_run TIMESTAMPTZ,
  previous_run TIMESTAMPTZ,
  status text NOT NULL default 'pending',
  processing_started_at TIMESTAMPTZ,
  scouter_version text not null default '0.1.0'
);

CREATE INDEX idx_drift_profile_job_queue 
ON scouter.drift_profile (next_run)
WHERE active = true;

CREATE INDEX idx_drift_profile_status 
ON scouter.drift_profile (status, next_run)
WHERE active = true;


-- Run maintenance every hour
SELECT  cron.schedule('partition-maintenance', '0 * * * *', $$CALL scouter.run_maintenance_proc()$$);

-- Run maintenance once a day at midnight utc with p_analyze set to true
SELECT  cron.schedule('partition-maintenance-analyze', '30 0 * * *', $$CALL scouter.run_maintenance_proc(0, true, true)$$);