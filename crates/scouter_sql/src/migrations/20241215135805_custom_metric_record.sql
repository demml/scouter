CREATE TABLE IF NOT exists scouter.custom_metric (
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    name text not null,
    space text not null,
    version text not null,
    metric text not null,
    value double precision,
    UNIQUE (created_at,name,space,version)
)
PARTITION BY RANGE (created_at);

CREATE INDEX ON scouter.custom_metric (name, space, version, created_at, metric);

SELECT scouter.create_parent(
               'scouter.custom_metric',
               'created_at',
               '1 day'
);

UPDATE scouter.part_config SET retention = '7 days' WHERE parent_table = 'scouter.custom_metric';
