ALTER TABLE scouter.agent_eval_record
    ADD COLUMN IF NOT EXISTS record_source TEXT NOT NULL DEFAULT 'user';

ALTER TABLE scouter.agent_eval_record
    DROP CONSTRAINT IF EXISTS chk_agent_eval_record_source;

ALTER TABLE scouter.agent_eval_record
    ADD CONSTRAINT chk_agent_eval_record_source
        CHECK (record_source IN ('user', 'queue', 'trace_dispatch'));

UPDATE scouter.agent_eval_record
SET record_source = 'trace_dispatch'
WHERE record_id LIKE 'trace-dispatch:%';
