SELECT
    id,
    entity_id,
    entity_name,
    alert,
    active,
    created_at,
    updated_at
FROM scouter.drift_alert
WHERE
    entity_id = $1
    AND ($2::BOOLEAN IS NULL OR active = $2)
    AND ($7::TIMESTAMPTZ IS NULL OR created_at >= $7)  -- begin_datetime
    AND ($8::TIMESTAMPTZ IS NULL OR created_at < $8)   -- end_datetime
    AND (
        ($3::TIMESTAMP IS NULL) -- No cursor, get first page
        OR (
            CASE
                WHEN $4 = 'previous' THEN
                    (created_at, id) > ($3, $5) -- Backward pagination (ASC order)
                ELSE
                    (created_at, id) < ($3, $5) -- Forward pagination (DESC order)
            END
        )
    )
ORDER BY
    CASE
        WHEN $4 = 'previous' THEN created_at
    END ASC,
    CASE
        WHEN $4 = 'previous' THEN id
    END ASC,
    CASE
        WHEN $4 != 'previous' OR $4 IS NULL THEN created_at
    END DESC,
    CASE
        WHEN $4 != 'previous' OR $4 IS NULL THEN id
    END DESC
LIMIT $6 + 1