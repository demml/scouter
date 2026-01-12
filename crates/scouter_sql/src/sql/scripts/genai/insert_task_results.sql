INSERT INTO scouter.genai_eval_task (
    created_at,
    record_uid,
    entity_id,
    task_id,
    task_type,
    passed,
    value,
    field_path,
    operator,
    expected,
    actual,
    message,
    condition,
    stage
    )
SELECT
    created_at,
    record_uid,
    entity_id,
    task_id,
    task_type,
    passed,
    value,
    field_path,
    operator,
    expected,
    actual,
    message,
    condition,
    stage
FROM UNNEST(
    $1::timestamptz[], -- created_at
    $2::text[],        -- record_uid
    $3::integer[],     -- entity_id
    $4::text[],        -- task_id
    $5::text[],        -- task_type
    $6::boolean[],     -- passed
    $7::double precision[], -- value
    $8::text[],        -- field_path
    $9::text[],        -- operator
    $10::jsonb[],       -- expected
    $11::jsonb[],       -- actual
    $12::text[],        -- message
    $13::boolean[],     -- condition
    $14::integer[]    -- stage

) AS t(
    created_at,
    record_uid,
    entity_id,
    task_id,
    task_type,
    passed,
    value,
    field_path,
    operator,
    expected,
    actual,
    message,
    condition,
    stage
)
ON CONFLICT DO NOTHING;