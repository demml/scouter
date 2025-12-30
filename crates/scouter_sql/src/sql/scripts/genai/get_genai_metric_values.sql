SELECT metric,
       AVG(value) AS value
FROM scouter.genai_drift
WHERE 1=1
  AND created_at > $1
  AND entity_id = $2
  AND metric = ANY($3)
GROUP BY metric