-- Migrations

CREATE TABLE IF NOT exists scouter.drift (
  created_at timestamp not null default (timezone('utc', now())),
  name varchar(256),
  repository varchar(256),
  feature varchar(256),
  value double precision,
  version varchar(256),
  UNIQUE (created_at,name,repository,feature,value,version)
)
PARTITION BY RANGE (created_at);

CREATE INDEX ON scouter.drift (name, repository, version, created_at);

SELECT scouter.create_parent(
    'scouter.drift', 
    'created_at',
    '1 day'
);

UPDATE scouter.part_config SET retention = '7 days' WHERE parent_table = 'scouter.drift';

-- Create table for service drift configuration
CREATE table IF NOT exists scouter.drift_profile (
  created_at timestamp not null default (timezone('utc', now())),
  updated_at timestamp not null default (timezone('utc', now())),
  name varchar(256),
  repository varchar(256),
  version varchar(256),
  profile jsonb,
  active boolean default true,
  schedule  varchar(256),
  next_run timestamp,
  previous_run timestamp,
  PRIMARY KEY (name, repository, version)
);

-- Run maintenance every hour
SELECT  cron.schedule('partition-maintenance', '0 * * * *', $$CALL scouter.run_maintenance_proc()$$);

-- Run maintenance once a day at midnight utc with p_analyze set to true
SELECT  cron.schedule('partition-maintenance-analyze', '30 0 * * *', $$CALL scouter.run_maintenance_proc(0, true, true)$$);