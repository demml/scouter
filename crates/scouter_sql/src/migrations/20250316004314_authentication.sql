
CREATE TABLE IF NOT EXISTS scouter.user (
    id SERIAL PRIMARY KEY,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    active BOOLEAN DEFAULT TRUE,
    username VARCHAR(255) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    permissions JSONB NOT NULL,
    group_permissions JSONB NOT NULL,
    role VARCHAR(32) DEFAULT 'user',
    refresh_token VARCHAR(255),
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);
