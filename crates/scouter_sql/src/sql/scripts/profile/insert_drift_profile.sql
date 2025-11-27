-- Insert drift profile
WITH entity_insert AS (
    INSERT INTO scouter.entities (space, name, version)
    VALUES ($1, $2, $8)
    ON CONFLICT (space, name, version) 
    DO UPDATE SET space = EXCLUDED.space
    RETURNING id, uid
)
INSERT INTO scouter.drift_profile (
    uid,
    entity_id,
    space,
    name,
    major,
    minor,
    patch,
    pre_tag,
    build_tag,
    version,
    scouter_version,
    profile,
    drift_type,
    active,
    schedule,
    next_run,
    previous_run
)
SELECT 
    entity_insert.uid,
    entity_insert.id,
    $1,
    $2,
    $3,
    $4,
    $5,
    $6,
    $7,
    $8,
    $9,
    $10,
    $11,
    $12,
    $13,
    $14,
    $15
FROM entity_insert
ON CONFLICT DO NOTHING
RETURNING (SELECT uid FROM entity_insert) as entity_uid;