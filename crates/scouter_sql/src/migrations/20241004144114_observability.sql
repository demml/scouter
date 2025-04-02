-- Add migration script here
CREATE TABLE IF NOT exists scouter.observability_metrics (
  created_at timestamp not null default (timezone('utc', now())),
  space varchar(256) not null,
  name varchar(256) not null,
  version varchar(256) not null,
  request_count integer not null default 0,
  error_count integer not null default 0,
  route_metrics jsonb not null default '[]',
  UNIQUE (created_at,name,space,version)
)
PARTITION BY RANGE (created_at);

CREATE INDEX ON scouter.observability_metrics (name, space, version, created_at);

SELECT scouter.create_parent(
    'scouter.observability_metrics',
    'created_at',
    '1 day'
);

UPDATE scouter.part_config SET retention = '14 days' WHERE parent_table = 'scouter.observability_metrics';