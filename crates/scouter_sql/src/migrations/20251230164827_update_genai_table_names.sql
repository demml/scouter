
-- ============================================================================
-- STEP 1: Drop Existing Tables and Partitions
-- ============================================================================

-- Drop llm_drift_record and all its partitions
DROP TABLE IF EXISTS scouter.llm_drift_record CASCADE;

-- Drop llm_drift and all its partitions
DROP TABLE IF EXISTS scouter.llm_drift CASCADE;

-- ============================================================================
-- STEP 2: Clean up pg_partman Configuration
-- ============================================================================

-- Remove pg_partman configurations for dropped tables
DELETE FROM scouter.part_config
WHERE parent_table IN ('scouter.llm_drift_record', 'scouter.llm_drift');

-- ============================================================================
-- STEP 3: Clean up Cron Jobs
-- ============================================================================

-- Remove any cron jobs related to the old tables
DELETE FROM cron.job
WHERE command LIKE '%llm_drift%';

-- ============================================================================
-- STEP 4: Drop Template Tables (if they exist)
-- ============================================================================

DROP TABLE IF EXISTS scouter.template_scouter_llm_drift_record CASCADE;
DROP TABLE IF EXISTS scouter.template_scouter_llm_drift CASCADE;

-- ============================================================================
-- STEP 5: Create New genai_event_record Table
-- ============================================================================

CREATE TABLE IF NOT EXISTS scouter.genai_eval_record (
    id BIGINT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    uid TEXT DEFAULT gen_random_uuid(),
    entity_id INTEGER NOT NULL,
    context JSONB,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending', -- pending, processing, completed, failed
    processing_started_at TIMESTAMPTZ,
    processing_ended_at TIMESTAMPTZ,
    processing_duration INTEGER,
    record_id TEXT,
    archived BOOLEAN DEFAULT false,
    PRIMARY KEY (uid, created_at),
    UNIQUE (created_at, name, space, version)
)
PARTITION BY RANGE (created_at);

-- ============================================================================
-- STEP 6: Create Indexes
-- ============================================================================
CREATE INDEX IF NOT EXISTS idx_genai_event_record_lookup ON scouter.genai_eval_record (entity_id, created_at);
CREATE INDEX IF NOT EXISTS idx_genai_event_record_pagination ON scouter.genai_eval_record (entity_id, id DESC);;


-- ============================================================================
-- STEP 7: Setup pg_partman Configuration
-- ============================================================================

-- Register table with pg_partman for automatic partition management
SELECT scouter.create_parent(
    'scouter.genai_eval_record',
    'created_at',
    '1 day'
);

-- Set retention policy to automatically drop old partitions after 60 days
UPDATE scouter.part_config
SET retention = '60 days'
WHERE parent_table = 'scouter.genai_eval_record';