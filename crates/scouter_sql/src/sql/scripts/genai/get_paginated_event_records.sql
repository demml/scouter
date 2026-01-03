SELECT
    uid,
    created_at,
    context,
    prompt,
    status,
    score,
    id,
    updated_at,
    processing_started_at,
    processing_ended_at,
    processing_duration,
    entity_id
FROM scouter.genai_event_record
WHERE entity_id = $1
  AND ($2::VARCHAR IS NULL OR status = $2)
  AND ($7::TIMESTAMPTZ IS NULL OR created_at >= $7)  -- start_datetime
  AND ($8::TIMESTAMPTZ IS NULL OR created_at < $8)   -- end_datetime

  AND (
    $3::TIMESTAMPTZ IS NULL OR
    (
      CASE
        WHEN $4 = 'previous' THEN (created_at, id) > ($3, $5)
        ELSE (created_at, id) < ($3, $5)
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
    WHEN $4 != 'previous' THEN created_at
  END DESC,
  CASE
    WHEN $4 != 'previous' THEN id
  END DESC
LIMIT $6 + 1;