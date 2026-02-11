-- drift entities table
CREATE TABLE IF NOT EXISTS scouter.drift_entities (
    id SERIAL PRIMARY KEY,
    uid TEXT UNIQUE DEFAULT gen_random_uuid(),
    space TEXT NOT NULL,
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    drift_type TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE (space, name, version, drift_type)
);

CREATE INDEX IF NOT EXISTS idx_entities_lookup ON scouter.drift_entities (space, name, version, drift_type);

INSERT INTO scouter.drift_entities (space, name, version, drift_type)
SELECT DISTINCT space, name, version, drift_type FROM scouter.drift_profile
ON CONFLICT (space, name, version, drift_type) DO NOTHING;

UPDATE scouter.drift_entities e
SET uid = dp.uid
FROM scouter.drift_profile dp
WHERE e.space = dp.space AND e.name = dp.name AND e.version = dp.version;


DO $$
DECLARE
    entity_tables TEXT[] := ARRAY[
        'scouter.drift_profile',
        'scouter.observability_metric',
        'scouter.llm_drift',
        'scouter.llm_drift_record',
        'scouter.spc_drift',
        'scouter.drift_alert',
        'scouter.custom_drift',
        'scouter.psi_drift'
    ]::TEXT[];

    entity_template_tables TEXT[] := ARRAY[
        'scouter.template_scouter_custom_drift',
        'scouter.template_scouter_drift_alert',
        'scouter.template_scouter_llm_drift',
        'scouter.template_scouter_llm_drift_record',
        'scouter.template_scouter_observability_metric',
        'scouter.template_scouter_psi_drift',
        'scouter.template_scouter_spc_drift'
    ]::TEXT[];

    tbl TEXT;
    has_drift_type BOOLEAN;
    join_condition TEXT;
BEGIN
    -- Handle Entity Tables
    FOREACH tbl IN ARRAY entity_tables LOOP
        RAISE NOTICE 'Migrating Entity Table: %', tbl;

        -- 1. Add entity_id column
        EXECUTE format('ALTER TABLE %s ADD COLUMN IF NOT EXISTS entity_id INTEGER', tbl);

        -- 2. Check if table has drift_type column
        SELECT EXISTS (
            SELECT 1
            FROM information_schema.columns
            WHERE table_schema = split_part(tbl, '.', 1)
              AND table_name = split_part(tbl, '.', 2)
              AND column_name = 'drift_type'
        ) INTO has_drift_type;

        -- 3. Determine Join Condition based on column existence
        IF has_drift_type THEN
            join_condition := 'AND COALESCE(t.drift_type, '''') = COALESCE(e.drift_type, '''')';
        ELSE
            join_condition := 'AND e.drift_type IS NULL';
        END IF;

        -- 4. Backfill (Using Dynamic Join Condition)
        EXECUTE format($q$
            UPDATE %s t
            SET entity_id = e.id
            FROM scouter.drift_entities e
            WHERE t.space = e.space
              AND t.name = e.name
              AND t.version = e.version
              %s -- Injected Condition
              AND t.entity_id IS NULL
        $q$, tbl, join_condition);

        -- 5. Delete bad data and enforce NOT NULL
        EXECUTE format('DELETE FROM %s WHERE entity_id IS NULL', tbl);
        EXECUTE format('ALTER TABLE %s ALTER COLUMN entity_id SET NOT NULL', tbl);

        -- 6. Drop Old Columns
        BEGIN
            -- skip if scoute.drift_profile table (want to keep space/name for profile purposes)
            IF tbl != 'scouter.drift_profile' THEN
                EXECUTE format('ALTER TABLE %s DROP COLUMN IF EXISTS space, DROP COLUMN IF EXISTS name, DROP COLUMN IF EXISTS version CASCADE, DROP COLUMN IF EXISTS drift_type', tbl);
            END IF;
        EXCEPTION WHEN OTHERS THEN
            RAISE NOTICE 'Constraints dropping handled via CASCADE or manual cleanup for %', tbl;
        END;
    END LOOP;

    -- Handle Entity Template Tables
    FOREACH tbl IN ARRAY entity_template_tables LOOP
        RAISE NOTICE 'Updating Entity Template: %', tbl;
        EXECUTE format('ALTER TABLE %s ADD COLUMN IF NOT EXISTS entity_id INTEGER NOT NULL', tbl);
        EXECUTE format('ALTER TABLE %s DROP COLUMN IF EXISTS space, DROP COLUMN IF EXISTS name, DROP COLUMN IF EXISTS version CASCADE, DROP COLUMN IF EXISTS drift_type', tbl);
    END LOOP;

END $$;

-- 2.5: Add entity_uid to LLM tables
ALTER TABLE scouter.llm_drift RENAME COLUMN record_uid TO uid;


-- OBSERVABILITY METRIC
DROP INDEX IF EXISTS scouter.observability_metric_created_at_space_name_version_idx;
ALTER TABLE scouter.observability_metric DROP CONSTRAINT IF EXISTS observability_metric_created_at_name_space_version_key;
ALTER TABLE scouter.observability_metric ADD UNIQUE (created_at, entity_id);

-- LLM DRIFT
DROP INDEX IF EXISTS scouter.idx_llm_drift_created_at_space_name_version_metric;
ALTER TABLE scouter.llm_drift DROP CONSTRAINT IF EXISTS llm_drift_created_at_space_name_version_key;
ALTER TABLE scouter.llm_drift ADD UNIQUE (created_at, entity_id);

-- LLM DRIFT RECORD
DROP INDEX IF EXISTS scouter.idx_llm_drift_record_created_at_space_name_version;
DROP INDEX IF EXISTS scouter.idx_llm_drift_record_pagination;
ALTER TABLE scouter.llm_drift_record DROP CONSTRAINT IF EXISTS llm_drift_record_created_at_name_space_version_key;

-- SPC DRIFT
DROP INDEX IF EXISTS scouter.idx_spc_drift_created_at_space_name_version_feature;
ALTER TABLE scouter.spc_drift DROP CONSTRAINT IF EXISTS spc_drift_created_at_name_space_feature_value_version_key;
ALTER TABLE scouter.spc_drift ADD UNIQUE (created_at, entity_id, feature, value);

-- CUSTOM DRIFT
DROP INDEX IF EXISTS scouter.idx_custom_drift_created_at_space_name_version_metric;
ALTER TABLE scouter.custom_drift DROP CONSTRAINT IF EXISTS custom_drift_created_at_name_space_version_key;
ALTER TABLE scouter.custom_drift ADD UNIQUE (created_at, entity_id);

-- PSI DRIFT
DROP INDEX IF EXISTS scouter.idx_psi_drift_created_at_space_name_version_feature;
ALTER TABLE scouter.psi_drift DROP CONSTRAINT IF EXISTS psi_drift_created_at_name_space_version_feature_bin_id_key;
ALTER TABLE scouter.psi_drift ADD UNIQUE (created_at, entity_id, feature, bin_id);

-- DRIFT ALERT
DROP INDEX IF EXISTS scouter.idx_drift_alert_created_at_space_name_version;
ALTER TABLE scouter.drift_alert DROP CONSTRAINT IF EXISTS drift_alert_created_at_name_space_version_key;
ALTER TABLE scouter.drift_alert ADD UNIQUE (entity_id, created_at);


-- Drift records
CREATE INDEX IF NOT EXISTS idx_llm_drift_lookup ON scouter.llm_drift (created_at, entity_id);
CREATE INDEX IF NOT EXISTS idx_llm_drift_record_lookup ON scouter.llm_drift_record (entity_id, created_at);
CREATE INDEX IF NOT EXISTS idx_llm_drift_record_pagination ON scouter.llm_drift_record (entity_id, id DESC);
CREATE INDEX IF NOT EXISTS idx_observability_lookup ON scouter.observability_metric (created_at, entity_id);