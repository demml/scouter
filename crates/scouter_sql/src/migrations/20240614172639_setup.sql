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
  UNIQUE (created_at,name,space,feature,value,version)
)
PARTITION BY RANGE (created_at);

CREATE INDEX ON scouter.spc_drift (name, space, version, created_at);

SELECT scouter.create_parent(
    'scouter.spc_drift',
    'created_at',
    '1 day'
);

UPDATE scouter.part_config SET retention = '7 days' WHERE parent_table = 'scouter.spc_drift';

-- Create table for service drift configuration
CREATE table IF NOT exists scouter.drift_profile (
  created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
  updated_at TIMESTAMPTZ DEFAULT NOW(),
  name text,
  space text,
  version text,
  drift_type text,
  profile jsonb,
  active boolean default true,
  schedule  text,
  next_run TIMESTAMPTZ,
  previous_run TIMESTAMPTZ,
  scouter_version text not null default '0.1.0',
  PRIMARY KEY (name, space, version, drift_type)
);

-- Run maintenance every hour
SELECT  cron.schedule('partition-maintenance', '0 * * * *', $$CALL scouter.run_maintenance_proc()$$);

-- Run maintenance once a day at midnight utc with p_analyze set to true
SELECT  cron.schedule('partition-maintenance-analyze', '30 0 * * *', $$CALL scouter.run_maintenance_proc(0, true, true)$$);