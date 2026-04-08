-- Rename genai eval tables to agent eval tables
ALTER TABLE scouter.genai_eval_record RENAME TO agent_eval_record;
ALTER TABLE scouter.genai_eval_workflow RENAME TO agent_eval_workflow;
ALTER TABLE scouter.genai_eval_task RENAME TO agent_eval_task;

-- Rename template tables
ALTER TABLE scouter.template_scouter_genai_eval_record RENAME TO template_scouter_agent_eval_record;
ALTER TABLE scouter.template_scouter_genai_eval_task RENAME TO template_scouter_agent_eval_task;
ALTER TABLE scouter.template_scouter_genai_eval_workflow RENAME TO template_scouter_agent_eval_workflow;

-- Update drift_type values
UPDATE scouter.drift_profile SET drift_type = 'agent' WHERE drift_type = 'genai';
UPDATE scouter.drift_entities SET drift_type = 'agent' WHERE drift_type = 'genai';

-- Update entity_type in tags table
UPDATE scouter.tags SET entity_type = 'Agent' WHERE entity_type = 'GenAI';
