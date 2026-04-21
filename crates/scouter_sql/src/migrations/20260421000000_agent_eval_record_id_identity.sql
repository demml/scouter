CREATE SEQUENCE IF NOT EXISTS scouter.agent_eval_record_id_seq;

ALTER TABLE scouter.agent_eval_record
    ALTER COLUMN id SET DEFAULT nextval('scouter.agent_eval_record_id_seq');
