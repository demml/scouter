CREATE TABLE IF NOT exists scouter.psi_drift (
  created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
  name text not null,
  space text not null,
  version text not null,
  feature text not null,
  bin_id integer not null,
  bin_count integer not null,
  updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
  archived boolean default false,
  UNIQUE (created_at,name,space,version,feature,bin_id)
)
PARTITION BY RANGE (created_at);

CREATE INDEX ON scouter.psi_drift (name, space, version, created_at, feature);

SELECT scouter.create_parent(
               'scouter.psi_drift',
               'created_at',
               '1 day'
);

UPDATE scouter.part_config SET retention = '60 days' WHERE parent_table = 'scouter.psi_drift';