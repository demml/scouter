INSERT INTO scouter.llm_drift_record (name, space, version, input, output, prompt)
VALUES ($1, $2, $3, $4, $5, $6)
ON CONFLICT DO NOTHING;
