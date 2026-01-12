SELECT
    id,
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
  AND ($6::TIMESTAMPTZ IS NULL OR created_at >= $6)  -- start_datetime
  AND ($7::TIMESTAMPTZ IS NULL OR created_at < $7)   -- end_datetime

  AND (
    $2::TIMESTAMPTZ IS NULL OR
    (
      CASE
        WHEN $3 = 'previous' THEN (created_at, id) > ($2, $4)
        ELSE (created_at,id) < ($2, $4)
      END
    )
  )
ORDER BY
  CASE
    WHEN $3 = 'previous' THEN created_at
  END ASC,
  CASE
    WHEN $3 = 'previous' THEN id
  END ASC,
  CASE
    WHEN $3 != 'previous' THEN created_at
  END DESC,
  CASE
    WHEN $3 != 'previous' THEN id
  END DESC
LIMIT $5 + 1;