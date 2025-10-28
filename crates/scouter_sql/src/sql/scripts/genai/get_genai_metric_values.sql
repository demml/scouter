SELECT metric,
       AVG(value) AS value
FROM scouter.llm_drift
WHERE 1=1
  AND created_at > $1
  AND space = $2
  AND name = $3
  AND version = $4
  AND metric = ANY($5)
GROUP BY metric