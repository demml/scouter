INSERT INTO scouter.genai_eval_task (
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
    $1::timestamptz[], -- start_time
    $2::timestamptz[], -- end_time
    $3::text[],        -- record_uid
    $4::integer[],     -- entity_id
    $5::text[],        -- task_id
    $6::text[],        -- task_type
    $7::boolean[],     -- passed
    $8::double precision[], -- value
    $9::text[],        -- field_path
    $10::text[],        -- operator
    $11::jsonb[],       -- expected
    $12::jsonb[],       -- actual
    $13::text[],        -- message
    $14::boolean[],     -- condition
    $15::integer[]    -- stage

) AS t(
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