BEGIN;

-- ============================================================================
-- STEP 1: Rename Main Tables
-- ============================================================================

-- Rename llm_drift_record to genai_event_record
ALTER TABLE scouter.llm_drift_record
RENAME TO genai_event_record;

-- Rename llm_drift to genai_drift
ALTER TABLE scouter.llm_drift
RENAME TO genai_drift;

-- ============================================================================
-- STEP 2: Update pg_partman Configuration
-- ============================================================================

UPDATE scouter.part_config
SET parent_table = 'scouter.genai_event_record'
WHERE parent_table = 'scouter.llm_drift_record';

UPDATE scouter.part_config
SET parent_table = 'scouter.genai_drift'
WHERE parent_table = 'scouter.llm_drift';

-- ============================================================================
-- STEP 3: Update Cron Jobs
-- ============================================================================

UPDATE cron.job
SET command = REPLACE(
    REPLACE(command, 'llm_drift_record', 'genai_event_record'),
    'llm_drift', 'genai_drift'
)
WHERE command LIKE '%llm_drift%';

-- ============================================================================
-- STEP 4: Rename Child Partitions
-- ============================================================================

DO $$
DECLARE
    partition_rec RECORD;
    new_partition_name TEXT;
BEGIN
    -- Rename llm_drift_record partitions to genai_event_record
    FOR partition_rec IN
        SELECT tablename
        FROM pg_tables
        WHERE schemaname = 'scouter'
          AND tablename LIKE 'llm_drift_record_p%'
    LOOP
        new_partition_name := REPLACE(partition_rec.tablename, 'llm_drift_record', 'genai_event_record');
        EXECUTE format('ALTER TABLE scouter.%I RENAME TO %I',
            partition_rec.tablename,
            new_partition_name
        );
        RAISE NOTICE 'Renamed partition: % -> %', partition_rec.tablename, new_partition_name;
    END LOOP;

    -- Rename llm_drift partitions to genai_drift
    FOR partition_rec IN
        SELECT tablename
        FROM pg_tables
        WHERE schemaname = 'scouter'
          AND tablename LIKE 'llm_drift_p%'
          AND tablename NOT LIKE 'llm_drift_record%'  -- Exclude already renamed
    LOOP
        new_partition_name := REPLACE(partition_rec.tablename, 'llm_drift', 'genai_drift');
        EXECUTE format('ALTER TABLE scouter.%I RENAME TO %I',
            partition_rec.tablename,
            new_partition_name
        );
        RAISE NOTICE 'Renamed partition: % -> %', partition_rec.tablename, new_partition_name;
    END LOOP;
END $$;

-- ============================================================================
-- STEP 5: Rename Template Tables (pg_partman templates)
-- ============================================================================

ALTER TABLE IF EXISTS scouter.template_scouter_llm_drift_record
RENAME TO template_scouter_genai_event_record;

ALTER TABLE IF EXISTS scouter.template_scouter_llm_drift
RENAME TO template_scouter_genai_drift;

-- ============================================================================
-- STEP 6: Update part_config Template Table References
-- ============================================================================

UPDATE scouter.part_config
SET template_table = 'scouter.template_scouter_genai_event_record'
WHERE template_table = 'scouter.template_scouter_llm_drift_record';

UPDATE scouter.part_config
SET template_table = 'scouter.template_scouter_genai_drift'
WHERE template_table = 'scouter.template_scouter_llm_drift';

-- ============================================================================
-- STEP 7: Rename Indexes
-- ============================================================================

-- Rename llm_drift indexes to genai_drift
ALTER INDEX IF EXISTS scouter.idx_llm_drift_lookup
RENAME TO idx_genai_drift_lookup;

ALTER INDEX IF EXISTS scouter.llm_drift_created_at_entity_id_key
RENAME TO genai_drift_created_at_entity_id_key;

-- Rename llm_drift_record indexes to genai_event_record
ALTER INDEX IF EXISTS scouter.idx_llm_drift_record_lookup
RENAME TO idx_genai_event_record_lookup;

ALTER INDEX IF EXISTS scouter.idx_llm_drift_record_pagination
RENAME TO idx_genai_event_record_pagination;

ALTER INDEX IF EXISTS scouter.idx_llm_drift_record_status
RENAME TO idx_genai_event_record_status;

-- ============================================================================
-- STEP 8: Update drift_entities Array Reference
-- ============================================================================

-- Update the entity_tables array in the entities migration if needed
DO $$
DECLARE
    migration_content TEXT;
BEGIN
    -- This is informational only - you'll need to update your entities migration manually
    RAISE NOTICE 'Remember to update entity_tables array in 20251105200007_entities.sql';
    RAISE NOTICE 'Change: scouter.llm_drift -> scouter.genai_drift';
    RAISE NOTICE 'Change: scouter.llm_drift_record -> scouter.genai_event_record';
END $$;

-- ============================================================================
-- STEP 9: Verification
-- ============================================================================

SELECT 'Part config after rename:' as info;
SELECT parent_table, partition_type, retention, template_table
FROM scouter.part_config
WHERE parent_table LIKE 'scouter.genai%';

SELECT 'Partitions after rename:' as info;
SELECT schemaname, tablename
FROM pg_tables
WHERE schemaname = 'scouter'
  AND (tablename LIKE 'genai_%' OR tablename LIKE '%genai%')
ORDER BY tablename;

SELECT 'Template tables after rename:' as info;
SELECT tablename
FROM pg_tables
WHERE schemaname = 'scouter'
  AND tablename LIKE 'template_scouter_genai%';

SELECT 'Indexes after rename:' as info;
SELECT indexname, tablename
FROM pg_indexes
WHERE schemaname = 'scouter'
  AND (indexname LIKE '%genai%' OR tablename LIKE '%genai%')
ORDER BY tablename, indexname;

COMMIT;