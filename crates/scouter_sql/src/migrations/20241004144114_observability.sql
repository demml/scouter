-- Add migration script here
CREATE TABLE IF NOT exists observability_metrics (
  created_at timestamp not null default (timezone('utc', now())),
  repository varchar(256) not null,
  name varchar(256) not null,
  version varchar(256) not null,
  request_count integer not null default 0,
  error_count integer not null default 0,
  route_metrics jsonb not null default '[]',
  UNIQUE (created_at,name,repository,version)
)
PARTITION BY RANGE (created_at);

CREATE INDEX ON observability_metrics (name, repository, version, created_at);

SELECT create_parent(
    'public.observability_metrics', 
    'created_at',
    '1 day'
);

UPDATE part_config SET retention = '14 days' WHERE parent_table = 'observability_metrics';