INSERT INTO scouter.genai_eval_workflow (
    created_at,
    record_uid,
    entity_id,
    total_tasks,
    passed_tasks,
    failed_tasks,
    pass_rate,
    duration_ms
    )
SELECT
    created_at,
    record_uid,
    entity_id,
    total_tasks,
    passed_tasks,
    failed_tasks,
    pass_rate,
    duration_ms
FROM UNNEST(
    $1::timestamptz[], -- created_at
    $2::text[],        -- record_uid
    $3::integer[],     -- entity_id
    $4::integer[],     -- total_tasks
    $5::integer[],     -- passed_tasks
    $6::integer[],     -- failed_tasks
    $7::double precision[], -- pass_rate
    $8::integer[]        -- duration_ms
) AS t(
    created_at,
    record_uid,
    entity_id,
    total_tasks,
    passed_tasks,
    failed_tasks,
    pass_rate,
    duration_ms
)
ON CONFLICT DO NOTHING;