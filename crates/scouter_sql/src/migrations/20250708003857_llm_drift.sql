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
    UNIQUE (created_at, name, space, version)
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
    id BIGSERIAL,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    uid text DEFAULT gen_random_uuid(),
    name text not null,
    space text not null,
    version text not null,
    input text not null,
    response text not null,
    context jsonb not null,
    prompt jsonb not null,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    status text NOT NULL default 'pending', -- pending, processing, completed, failed
    processing_started_at TIMESTAMPTZ,
    processing_ended_at TIMESTAMPTZ,
    PRIMARY KEY (id, created_at),
    UNIQUE (created_at, name, space, version)
)
PARTITION BY RANGE (created_at);

CREATE INDEX idx_llm_drift_record_created_at_space_name_version
ON scouter.llm_drift_record (space, name, version, created_at);

-- process oldest pending records first
CREATE INDEX idx_llm_drift_record_status
ON scouter.llm_drift_record (status, created_at ASC)
WHERE status = 'pending';

CREATE INDEX idx_llm_drift_record_pagination 
ON scouter.llm_drift_record (space, name, version, id DESC);


SELECT scouter.create_parent(
               'scouter.llm_drift_record',
               'created_at',
               '1 day'
);
UPDATE scouter.part_config SET retention = '60 days' WHERE parent_table = 'scouter.llm_drift_record';