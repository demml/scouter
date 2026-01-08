SELECT 
  task_id as metric,
  AVG(value) AS value
FROM scouter.genai_eval_task
WHERE 1=1
  AND created_at > $1
  AND entity_id = $2
  AND task_id = ANY($3)
GROUP BY task_id