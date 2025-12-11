-- Add migration script here
-- =================================================================
-- Simple pg_partman Configuration Update
-- =================================================================

DELETE FROM scouter.part_config WHERE parent_table = 'scouter.spans';
DELETE FROM scouter.part_config WHERE parent_table = 'scouter.trace_baggage';
DELETE FROM scouter.part_config WHERE parent_table = 'scouter.tags';

SELECT scouter.create_parent(
    'scouter.spans',
    'created_at',
    '1 day'
);

UPDATE scouter.part_config
SET
    retention = '30 days',
    optimize_constraint = 10,
    retention_keep_table = FALSE
WHERE parent_table = 'scouter.spans';

SELECT scouter.create_parent(
    'scouter.trace_baggage',
    'created_at',
    '1 day'
);

UPDATE scouter.part_config
SET
    premake = 7,
    retention = '30 days',
    retention_keep_table = FALSE
WHERE parent_table = 'scouter.trace_baggage';

SELECT scouter.create_parent(
    'scouter.tags',
    'created_at',
    '7 days'
);

UPDATE scouter.part_config
SET
    premake = 4,
    retention = '90 days',
    retention_keep_table = FALSE
WHERE parent_table = 'scouter.tags';

UPDATE scouter.part_config SET retention_keep_table = FALSE WHERE parent_table = 'scouter.spc_drift';
UPDATE scouter.part_config SET retention_keep_table = FALSE WHERE parent_table = 'scouter.drift_alert';
UPDATE scouter.part_config SET retention_keep_table = FALSE WHERE parent_table = 'scouter.observability_metric';
UPDATE scouter.part_config SET retention_keep_table = FALSE WHERE parent_table = 'scouter.psi_drift';
UPDATE scouter.part_config SET retention_keep_table = FALSE WHERE parent_table = 'scouter.custom_drift';
UPDATE scouter.part_config SET retention_keep_table = FALSE WHERE parent_table = 'scouter.llm_drift';
UPDATE scouter.part_config SET retention_keep_table = FALSE WHERE parent_table = 'scouter.llm_drift_record';




