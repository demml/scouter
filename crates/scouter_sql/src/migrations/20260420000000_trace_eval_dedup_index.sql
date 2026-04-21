CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_eval_record_entity_trace
    ON scouter.agent_eval_record (entity_id, trace_id)
    WHERE trace_id IS NOT NULL;
