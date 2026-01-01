SELECT 
  task_id,
  AVG(value) AS value
FROM scouter.genai_eval_task_result
WHERE 1=1
  AND created_at > $1
  AND entity_id = $2
GROUP BY task_id