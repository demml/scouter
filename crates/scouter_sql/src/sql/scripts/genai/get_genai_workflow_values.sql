SELECT AVG(pass_rate) AS value
FROM scouter.genai_eval_workflow
WHERE 1=1
  AND created_at > $1
  AND entity_id = $2
GROUP BY metric