SELECT
    created_at,
    name, 
    space, 
    major, 
    minor, 
    patch, 
    pre_tag, 
    build_tag, 
    uid
FROM scouter.drift_profile
WHERE 1=1
    AND space = $1
    AND name = $2