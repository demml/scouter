WITH next_id AS (
    SELECT COALESCE(MAX(id), 0) + 1 AS id
    FROM scouter.genai_eval_workflow
    WHERE entity_id = $3
)

INSERT INTO scouter.genai_eval_workflow (
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
)
VALUES (
    next_id.id,
    $1,  -- created_at: timestamptz
    $2,  -- record_uid: text
    $3,  -- entity_id: integer
    $4,  -- total_tasks: integer
    $5,  -- passed_tasks: integer
    $6,  -- failed_tasks: integer
    $7,  -- pass_rate: double precision
    $8,  -- duration_ms: integer
    $9   -- execution_plan: jsonb
)
ON CONFLICT DO NOTHING;