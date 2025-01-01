INSERT INTO drift (created_at, name, repository, version, feature, value) 
VALUES ($1, $2, $3, $4, $5, $6)
ON CONFLICT DO NOTHING;