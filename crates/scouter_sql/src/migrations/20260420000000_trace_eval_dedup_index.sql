-- `agent_eval_record` is partitioned by `created_at`, so a global UNIQUE index
-- on `(entity_id, trace_id)` is invalid unless it also includes the partition key.
-- Synthetic trace-eval dedup now uses deterministic keys + advisory locking in
-- application logic; this migration must be a safe no-op on all environments.
DROP INDEX IF EXISTS scouter.idx_agent_eval_record_entity_trace;
