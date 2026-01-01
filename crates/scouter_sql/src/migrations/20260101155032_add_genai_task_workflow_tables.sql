-- ============================================================================
-- Evaluation Workflow Summary Table
-- ============================================================================

CREATE TABLE IF NOT EXISTS scouter.genai_eval_workflow (
    created_at TIMESTAMPTZ NOT NULL,
    record_uid TEXT NOT NULL,
    entity_id INTEGER NOT NULL,
    total_tasks INTEGER NOT NULL,
    passed_tasks INTEGER NOT NULL,
    failed_tasks INTEGER NOT NULL,
    pass_rate DOUBLE PRECISION NOT NULL,
    duration_ms BIGINT,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    archived BOOLEAN DEFAULT FALSE,
    PRIMARY KEY (created_at, record_uid),
    CONSTRAINT fk_entity FOREIGN KEY (entity_id) REFERENCES scouter.entities(id) ON DELETE CASCADE
)
PARTITION BY RANGE (created_at);

CREATE INDEX idx_genai_eval_workflow_lookup
ON scouter.genai_eval_workflow (created_at, entity_id DESC);

-- partition
SELECT scouter.create_parent(
    'scouter.genai_eval_workflow',
    'created_at',
    '1 day'
);

UPDATE scouter.part_config
SET retention = '90 days'
WHERE parent_table = 'scouter.genai_eval_workflow';


CREATE TABLE IF NOT EXISTS scouter.genai_eval_task_result (
    created_at TIMESTAMPTZ NOT NULL,
    record_uid TEXT NOT NULL,
    entity_id INTEGER NOT NULL,
    task_id TEXT NOT NULL,
    task_type TEXT NOT NULL,
    passed BOOLEAN NOT NULL,
    value DOUBLE PRECISION NOT NULL,
    field_path TEXT,
    operator TEXT NOT NULL,
    expected JSONB NOT NULL,
    actual JSONB NOT NULL,
    message TEXT,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    archived BOOLEAN DEFAULT FALSE,
    PRIMARY KEY (record_uid, task_id, created_at)
)
PARTITION BY RANGE (created_at);

CREATE INDEX idx_genai_eval_task_record_lookup
ON scouter.genai_eval_task_result (created_at, record_uid DESC);

CREATE INDEX idx_genai_eval_entity_id_lookup
ON scouter.genai_eval_task_result (created_at, entity_id DESC);

-- Setup partitioning
SELECT scouter.create_parent(
    'scouter.genai_eval_task_result',
    'created_at',
    '1 day'
);

UPDATE scouter.part_config
SET retention = '90 days'
WHERE parent_table = 'scouter.genai_eval_task_result'

-- Need to drop old genai_drift table if it exists
DROP TABLE IF EXISTS scouter.genai_drift CASCADE;

-- drop genai_drift part_config entry
DELETE FROM scouter.part_config WHERE parent_table = 'scouter.genai_drift';

-- delete all partitions of genai_drift
DO $$
DECLARE
    partition_record RECORD;
BEGIN
    FOR partition_record IN
        SELECT tablename
        FROM pg_tables
        WHERE schemaname = 'scouter'
          AND tablename LIKE 'genai_drift_%'
    LOOP
        EXECUTE format('DROP TABLE IF EXISTS scouter.%I CASCADE', partition_record.tablename);
        RAISE NOTICE 'Dropped partition table: %', partition_record.tablename;
    END LOOP;
END $$;