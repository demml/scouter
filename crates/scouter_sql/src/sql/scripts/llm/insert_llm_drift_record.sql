INSERT INTO scouter.llm_drift_record (created_at, space, name, version, input, response, context, prompt)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
ON CONFLICT DO NOTHING;
