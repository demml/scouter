CREATE TABLE IF NOT exists scouter.custom_metrics (
    created_at timestamp not null default (timezone('utc', now())),
    name varchar(256) not null,
    repository varchar(256) not null,
    version varchar(256) not null,
    metric varchar(256) not null,
    value double precision,
    UNIQUE (created_at,name,repository,version)
)
PARTITION BY RANGE (created_at);

CREATE INDEX ON scouter.custom_metrics (name, repository, version, created_at, metric);

SELECT scouter.create_parent(
               'scouter.custom_metrics',
               'created_at',
               '1 day'
);

UPDATE scouter.part_config SET retention = '7 days' WHERE parent_table = 'scouter.custom_metrics';
