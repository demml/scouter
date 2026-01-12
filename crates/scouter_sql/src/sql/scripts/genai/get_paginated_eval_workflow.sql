SELECT
    created_at,
    record_uid,
    entity_id,
    total_tasks,
    passed_tasks,
    failed_tasks,
    pass_rate,
    duration_ms,
    execution_plan
FROM scouter.genai_eval_workflow
WHERE entity_id = $1
  AND ($2::VARCHAR IS NULL OR status = $2)
  AND ($7::TIMESTAMPTZ IS NULL OR created_at >= $7)  -- start_datetime
  AND ($8::TIMESTAMPTZ IS NULL OR created_at < $8)   -- end_datetime

  AND (
    $3::TIMESTAMPTZ IS NULL OR
    (
      CASE
        WHEN $4 = 'previous' THEN (created_at, record_uid) > ($3, $5)
        ELSE (created_at,record_uid) < ($3, $5)
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