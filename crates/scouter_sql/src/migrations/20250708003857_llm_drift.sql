-- Add migration script here
CREATE TABLE IF NOT exists scouter.llm_drift (
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

CREATE INDEX idx_llm_drift_created_at_space_name_version_metric
ON scouter.llm_drift (created_at, space, name, version, metric);

SELECT scouter.create_parent(
               'scouter.llm_drift',
               'created_at',
               '1 day'
);

UPDATE scouter.part_config SET retention = '60 days' WHERE parent_table = 'scouter.llm_drift';

-- Intermediary table for LLM Drift Records
-- When a server record is received, we insert it into this table.
-- Background job will process these records based on defined drift profiles and insert
-- metrics into the llm_drift table.
CREATE TABLE IF NOT exists scouter.llm_drift_record (
    id SERIAL PRIMARY KEY,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    name text not null,
    space text not null,
    version text not null,
    input text not null,
    response text not null,
    context jsonb not null,
    prompt jsonb not null,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    status text NOT NULL default 'pending', -- pending, processed
    UNIQUE (created_at,name,space,version)
)
PARTITION BY RANGE (created_at);

CREATE INDEX idx_llm_drift_record_created_at_space_name_version
ON scouter.llm_drift_record (created_at, space, name, version);

CREATE INDEX idx_llm_drift_record_status
ON scouter.llm_drift_record (status);

-- create index for when querying by space, name, version and id range
CREATE INDEX idx_llm_drift_record_space_name_version_id
ON scouter.llm_drift_record (space, name, version, id);

SELECT scouter.create_parent(
               'scouter.llm_drift_record',
               'created_at',
               '1 day'
);
UPDATE scouter.part_config SET retention = '60 days' WHERE parent_table = 'scouter.llm_drift_record';