INSERT INTO scouter.genai_eval_task (
    created_at,
    start_time,
    end_time,
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
    start_time,
    end_time,
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
    $2::timestamptz[], -- start_time
    $3::timestamptz[], -- end_time
    $4::text[],        -- record_uid
    $5::integer[],     -- entity_id
    $6::text[],        -- task_id
    $7::text[],        -- task_type
    $8::boolean[],     -- passed
    $9::double precision[], -- value
    $10::text[],        -- field_path
    $11::text[],        -- operator
    $12::jsonb[],       -- expected
    $13::jsonb[],       -- actual
    $14::text[],        -- message
    $15::boolean[],     -- condition
    $16::integer[]    -- stage
) AS t(
    created_at,
    start_time,
    end_time,
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