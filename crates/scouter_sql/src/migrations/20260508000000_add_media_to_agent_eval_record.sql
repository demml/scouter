ALTER TABLE scouter.agent_eval_record
    ADD COLUMN IF NOT EXISTS media JSONB NOT NULL DEFAULT '[]'::jsonb;
