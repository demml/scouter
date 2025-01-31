CREATE TABLE IF NOT exists scouter.observed_bin_count (
  created_at timestamp not null default (timezone('utc', now())),
  name varchar(256) not null,
  repository varchar(256) not null,
  version varchar(256) not null,
  feature varchar(256) not null,
  bin_id integer not null,
  bin_count integer not null,
  UNIQUE (created_at,name,repository,version,feature,bin_id)
)
PARTITION BY RANGE (created_at);

CREATE INDEX ON scouter.observed_bin_count (name, repository, version, created_at, feature);

SELECT scouter.create_parent(
               'scouter.observed_bin_count',
               'created_at',
               '1 day'
);

UPDATE scouter.part_config SET retention = '7 days' WHERE parent_table = 'scouter.observed_bin_count';