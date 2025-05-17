CREATE TABLE IF NOT exists scouter.custom_drift (
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    name text not null,
    space text not null,
    version text not null,
    metric text not null,
    value double precision,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    archived boolean default false,
    UNIQUE (created_at,name,space,version)
)
PARTITION BY RANGE (created_at);

CREATE INDEX idx_custom_drift_created_at_space_name_version_metric
ON scouter.custom_drift (created_at, space, name, version, metric);

SELECT scouter.create_parent(
               'scouter.custom_drift',
               'created_at',
               '1 day'
);

UPDATE scouter.part_config SET retention = '60 days' WHERE parent_table = 'scouter.custom_drift';
