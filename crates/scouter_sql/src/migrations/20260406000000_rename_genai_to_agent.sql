-- Rename GenAI eval tables to agent eval tables
ALTER TABLE scouter.genai_eval_record RENAME TO agent_eval_record;
ALTER TABLE scouter.genai_eval_workflow RENAME TO agent_eval_workflow;
ALTER TABLE scouter.genai_eval_task RENAME TO agent_eval_task;

-- Update drift_type column values in profile and entity tables
UPDATE scouter.drift_profile SET drift_type = 'agent' WHERE drift_type = 'genai';
UPDATE scouter.drift_entities SET drift_type = 'agent' WHERE drift_type = 'genai';
UPDATE scouter.alert SET drift_type = 'agent' WHERE drift_type = 'genai';

-- Update entity_type in tags table
UPDATE scouter.tags SET entity_type = 'Agent' WHERE entity_type = 'GenAI';
