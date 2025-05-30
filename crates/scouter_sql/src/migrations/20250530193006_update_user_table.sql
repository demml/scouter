-- need to receate the user table based on changes in opsml

-- Recreate user table with new schema
BEGIN;

-- First, rename the existing table as a backup
ALTER TABLE scouter.user RENAME TO user_old;

-- Create new table with updated schema
CREATE TABLE IF NOT EXISTS scouter.user (
    id SERIAL PRIMARY KEY,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    active BOOLEAN DEFAULT TRUE,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    hashed_recovery_codes JSONB NOT NULL,
    permissions JSONB NOT NULL,
    group_permissions JSONB NOT NULL,
    role TEXT DEFAULT 'user',
    favorite_spaces JSONB DEFAULT '[]',
    refresh_token TEXT,
    email TEXT NOT NULL UNIQUE,
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Copy existing data to new table with defaults for new columns
INSERT INTO scouter.user (
    id,
    created_at,
    active,
    username,
    password_hash,
    permissions,
    group_permissions,
    role,
    refresh_token,
    updated_at,
    hashed_recovery_codes,
    email,
    favorite_spaces
)
SELECT 
    id,
    created_at,
    active,
    username,
    password_hash,
    permissions,
    group_permissions,
    role,
    refresh_token,
    updated_at,
    '[]'::jsonb as hashed_recovery_codes,
    username || '@placeholder.com' as email,
    '[]'::jsonb as favorite_spaces
FROM scouter.user_old;

-- Drop the old table
DROP TABLE scouter.user_old;

COMMIT;