-- Refactoring llm_drift_record to genai_event_record table + adding tracing fields

BEGIN;

-- Rename existing table to preserve data
ALTER TABLE scouter.llm_drift_record RENAME TO genai_event_record;

-- Add new tracing and event columns
ALTER TABLE scouter.genai_event_record
ADD COLUMN IF NOT EXISTS trace_id text DEFAULT gen_random_uuid(),
ADD COLUMN IF NOT EXISTS parent_span_name text,
ADD COLUMN IF NOT EXISTS span_id text DEFAULT gen_random_uuid(),
ADD COLUMN IF NOT EXISTS span_name text,
ADD COLUMN IF NOT EXISTS inputs jsonb DEFAULT '{}',
ADD COLUMN IF NOT EXISTS outputs jsonb DEFAULT '{}',
ADD COLUMN IF NOT EXISTS ground_truth jsonb,
ADD COLUMN IF NOT EXISTS metadata jsonb DEFAULT '{}',
ADD COLUMN IF NOT EXISTS entity_type text DEFAULT 'genai';

-- Migrate existing data: map context -> inputs, set required defaults
UPDATE scouter.genai_event_record
SET 
    trace_id = COALESCE(trace_id, gen_random_uuid()),
    span_id = COALESCE(span_id, gen_random_uuid()),
    inputs = COALESCE(inputs, context, '{}'),
    outputs = COALESCE(outputs, '{}'),
    metadata = COALESCE(metadata, '{}'),
    entity_type = COALESCE(entity_type, 'llm')
WHERE trace_id IS NULL OR span_id IS NULL OR inputs IS NULL OR outputs IS NULL;

-- Set non-null constraints for required fields
ALTER TABLE scouter.genai_event_record
ALTER COLUMN trace_id SET NOT NULL,
ALTER COLUMN span_id SET NOT NULL,
ALTER COLUMN inputs SET NOT NULL,
ALTER COLUMN outputs SET NOT NULL,
ALTER COLUMN metadata SET NOT NULL,
ALTER COLUMN entity_type SET NOT NULL;

-- Drop old context column since we've migrated it to inputs
ALTER TABLE scouter.genai_event_record DROP COLUMN IF EXISTS context;

-- Remove old constraints before adding new ones
ALTER TABLE scouter.genai_event_record DROP CONSTRAINT IF EXISTS llm_drift_record_pkey;
ALTER TABLE scouter.genai_event_record DROP CONSTRAINT IF EXISTS llm_drift_record_created_at_name_space_version_key;

-- Create new primary key and constraints
ALTER TABLE scouter.genai_event_record 
ADD CONSTRAINT genai_event_record_pkey PRIMARY KEY (uid, created_at),
ADD CONSTRAINT genai_event_record_unique_span UNIQUE (span_id, created_at),
ADD CONSTRAINT genai_event_record_entity_type_check CHECK (entity_type IN ('llm', 'retrieval', 'embedding', 'classification'));

-- Drop old indexes
DROP INDEX IF EXISTS idx_llm_drift_record_created_at_space_name_version;
DROP INDEX IF EXISTS idx_llm_drift_record_status;
DROP INDEX IF EXISTS idx_llm_drift_record_pagination;

-- Create optimized indexes for trace reconstruction and LLM evaluation

-- Primary lookup for space/name/version filtering
CREATE INDEX idx_genai_event_record_space_name_version_time
ON scouter.genai_event_record (space, name, version, created_at DESC);

-- Critical for trace reconstruction - groups spans by trace
CREATE INDEX idx_genai_event_record_trace_reconstruction
ON scouter.genai_event_record (trace_id, created_at)
INCLUDE (span_id, span_name, parent_span_name, entity_type);

-- Span hierarchy navigation - find children of a span
CREATE INDEX idx_genai_event_record_span_hierarchy  
ON scouter.genai_event_record (parent_span_name, created_at)
WHERE parent_span_name IS NOT NULL;

-- Root span identification - entry points into traces
CREATE INDEX idx_genai_event_record_root_spans
ON scouter.genai_event_record (trace_id, created_at)
WHERE parent_span_name IS NULL;

-- Background processing - pending records
CREATE INDEX idx_genai_event_record_status_processing
ON scouter.genai_event_record (status, created_at ASC)
WHERE status IN ('pending', 'processing');

-- Entity type filtering for different AI operations
CREATE INDEX idx_genai_event_record_entity_type_filter
ON scouter.genai_event_record (entity_type, space, name, version, created_at DESC);

-- UI pagination with consistent ordering
CREATE INDEX idx_genai_event_record_pagination 
ON scouter.genai_event_record (space, name, version, created_at DESC, uid);

-- JSONB indexes for efficient metadata and input querying
CREATE INDEX idx_genai_event_record_metadata_gin
ON scouter.genai_event_record USING GIN (metadata);

CREATE INDEX idx_genai_event_record_inputs_gin
ON scouter.genai_event_record USING GIN (inputs);

-- Update partition configuration
UPDATE scouter.part_config 
SET parent_table = 'scouter.genai_event_record'
WHERE parent_table = 'scouter.llm_drift_record';

-- Update cron jobs to reference new table name
UPDATE cron.job 
SET command = REPLACE(command, 'llm_drift_record', 'genai_event_record')
WHERE command LIKE '%llm_drift_record%';

COMMIT;
-- End of migration