WITH entity_insert AS (
    -- 1. Get/Create the Entity ID
    INSERT INTO scouter.drift_entities (uid, space, name, version, drift_type)
    VALUES ($1, $2, $3, $9, $12)
    ON CONFLICT (space, name, version, drift_type)
    DO UPDATE SET uid = EXCLUDED.uid
    RETURNING id, uid
),
updated_profile AS (
    -- 2. Update the profile JSONB with the correct UID
    SELECT
        ei.id as entity_id,
        ei.uid as entity_uid,
        jsonb_set(
            $11::jsonb,
            '{config,uid}',
            to_jsonb(ei.uid)
        ) as profile_with_uid
    FROM entity_insert ei
),
deactivate_older AS (
    -- 3. Check if mass deactivation should occur
    SELECT 1 AS status
    WHERE $13 IS TRUE AND $17 IS TRUE
    LIMIT 1
),
deactivation_update AS (
    -- 4. Deactivate old profiles if conditions met
    UPDATE scouter.drift_profile dp
    SET active = FALSE,
        updated_at = CURRENT_TIMESTAMP
    FROM updated_profile up, deactivate_older d
    WHERE dp.entity_id IN (
        SELECT e_siblings.id
        FROM scouter.drift_entities e_siblings
        WHERE e_siblings.space = $2
          AND e_siblings.name = $3
          AND e_siblings.id != up.entity_id
    )
    AND dp.active IS TRUE
    AND dp.drift_type = $12
    RETURNING 1
)
-- 5. Insert new drift profile with updated JSONB
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
    up.entity_uid,
    up.entity_id,
    $2,  -- space
    $3,  -- name
    $4,  -- major
    $5,  -- minor
    $6,  -- patch
    $7,  -- pre_tag
    $8,  -- build_tag
    $9,  -- version
    $10, -- scouter_version
    up.profile_with_uid,
    $12, -- drift_type
    $13, -- active
    $14, -- schedule
    $15, -- next_run
    $16, -- previous_run
    $16, -- created_at
    $16  -- updated_at
FROM updated_profile up
ON CONFLICT (uid) DO NOTHING
RETURNING (SELECT uid FROM entity_insert) as entity_uid;