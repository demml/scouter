CREATE SEQUENCE IF NOT EXISTS scouter.agent_eval_record_id_seq;

ALTER TABLE scouter.agent_eval_record
    ALTER COLUMN id SET DEFAULT nextval('scouter.agent_eval_record_id_seq');

SELECT setval(
    'scouter.agent_eval_record_id_seq',
    COALESCE(MAX(id), 1),
    MAX(id) IS NOT NULL
)
FROM scouter.agent_eval_record;
