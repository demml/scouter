-- Add migration script here
CREATE TABLE IF NOT exists scouter.observability_metric (
  created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
  space text not null,
  name text not null,
  version text not null,
  request_count integer not null default 0,
  error_count integer not null default 0,
  route_metrics jsonb not null default '[]',
  updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
  UNIQUE (created_at,name,space,version)
)
PARTITION BY RANGE (created_at);

CREATE INDEX ON scouter.observability_metric (name, space, version, created_at);

SELECT scouter.create_parent(
    'scouter.observability_metric',
    'created_at',
    '1 day'
);

UPDATE scouter.part_config SET retention = '14 days' WHERE parent_table = 'scouter.observability_metric';