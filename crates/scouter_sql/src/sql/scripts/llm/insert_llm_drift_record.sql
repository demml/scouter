INSERT INTO scouter.llm_drift_record (space, name, version, input, response, context, prompt)
VALUES ($1, $2, $3, $4, $5, $6, $7)
ON CONFLICT DO NOTHING;
