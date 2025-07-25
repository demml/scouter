INSERT INTO scouter.llm_drift_record (created_at, space, name, version, context, prompt)
VALUES ($1, $2, $3, $4, $5, $6)
ON CONFLICT DO NOTHING;
