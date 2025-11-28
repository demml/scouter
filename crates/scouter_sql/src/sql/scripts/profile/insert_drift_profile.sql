-- Combined INSERT and DEACTIVATION (Queries::InsertDriftProfile)
WITH entity_insert AS (
    -- 1. Get/Create the Entity ID
    INSERT INTO scouter.entities (space, name, version, drift_type)
    VALUES ($1, $2, $8, $11)
    ON CONFLICT (space, name, version, drift_type) 
    DO UPDATE SET space = EXCLUDED.space
    RETURNING id, uid
),
deactivate_older AS (
    -- 2. Deactivate existing profiles if required ($12=active, $16=deactivate_others)
    -- This block only executes if the active flag is true and deactivate_others is true
    SELECT 1 AS status
    FROM scouter.entities e_current
    WHERE e_current.space = $1 AND e_current.name = $2
    AND $12 IS TRUE AND $16 IS TRUE -- Conditional execution logic
    LIMIT 1 -- Only need to execute this once
),
deactivation_update AS (
    -- 3. Perform the mass update if conditions are met
    UPDATE scouter.drift_profile dp
    SET active = FALSE,
        updated_at = CURRENT_TIMESTAMP
    FROM entity_insert ei, deactivate_older d
    WHERE dp.entity_id IN (
        -- Select all entity IDs matching the space/name of the new entity, 
        -- but excluding the new specific version's ID (which may not exist yet)
        SELECT id FROM scouter.entities e_siblings
        WHERE e_siblings.space = $1
          AND e_siblings.name = $2
          AND e_siblings.id != ei.id
    )
    AND dp.active IS TRUE
    AND dp.drift_type = $11
    RETURNING 1
)
-- 4. Final Insertion of the new drift profile
INSERT INTO scouter.drift_profile (
    uid,
    entity_id,
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
    previous_run,
    created_at,
    updated_at
)
SELECT 
    entity_insert.uid,
    entity_insert.id,
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
    $15,
    $15,
    $15
FROM entity_insert
ON CONFLICT DO NOTHING -- Should not conflict if entity_id is used.
RETURNING (SELECT uid FROM entity_insert) as entity_uid;