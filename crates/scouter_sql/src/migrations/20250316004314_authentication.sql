-- Add migration script here

CREATE TABLE IF NOT EXISTS scouter_users (
    id SERIAL PRIMARY KEY,
    created_at TIMESTAMP DEFAULT (TIMEZONE('utc', NOW())),
    active BOOLEAN DEFAULT TRUE,
    username VARCHAR(255) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    permissions JSONB NOT NULL,
    group_permissions JSONB NOT NULL,
    role VARCHAR(32) DEFAULT 'user',
    refresh_token VARCHAR(255)

);
