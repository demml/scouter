-- Combined INSERT and DEACTIVATION (Queries::InsertDriftProfile)
WITH entity_insert AS (
    -- 1. Get/Create the Entity ID
    INSERT INTO scouter.drift_entities (uid, space, name, version, drift_type)
    VALUES ($1, $2, $3, $9, $12) 
    ON CONFLICT (space, name, version, drift_type)
    DO UPDATE SET uid = EXCLUDED.uid
    RETURNING id, uid
),
deactivate_older AS (
    -- 2. Check if a mass deactivation needs to occur
    -- Uses $13 (active) and $17 (deactivate_others)
    SELECT 1 AS status
    FROM scouter.drift_entities e_current
    WHERE e_current.space = $2 AND e_current.name = $3
    AND $13 IS TRUE AND $17 IS TRUE -- Conditional execution logic
    LIMIT 1
),
deactivation_update AS (
    -- 3. Perform mass update if conditions are met
    UPDATE scouter.drift_profile dp
    SET active = FALSE,
        updated_at = CURRENT_TIMESTAMP
    FROM entity_insert ei, deactivate_older d
    WHERE dp.entity_id IN (
        -- Select all entity IDs matching the space/name of the new entity,
        SELECT id FROM scouter.drift_entities e_siblings
        WHERE e_siblings.space = $2
          AND e_siblings.name = $3
          AND e_siblings.id != ei.id -- Exclude the newly inserted/found entity's ID
    )
    AND dp.active IS TRUE
    AND dp.drift_type = $12 -- Filter by drift_type
    RETURNING 1
)
-- 4. Final Insertion of the new drift profile
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
    previous_run,
    created_at,
    updated_at
)
SELECT
    entity_insert.uid,
    entity_insert.id,
    $2, -- space
    $3, -- name
    $4,  -- major
    $5,  -- minor
    $6,  -- patch
    $7,  -- pre
    $8,  -- build
    $9,  -- version.to_string()
    $10, -- base_args.scouter_version
    $11, -- drift_profile.to_value()
    $12, -- drift_type.to_string()
    $13, -- active
    $14, -- base_args.schedule
    $15, -- next_run
    $16, -- current_time (used for previous_run, created_at, updated_at)
    $16,
    $16
FROM entity_insert
ON CONFLICT DO NOTHING
RETURNING (SELECT uid FROM entity_insert) as entity_uid;