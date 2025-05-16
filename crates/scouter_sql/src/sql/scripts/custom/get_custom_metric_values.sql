SELECT metric,
       AVG(value) AS value
FROM scouter.custom_drift
WHERE 1=1
  AND created_at > $4
  AND space = $2
  AND name = $1
  AND version = $3
  AND metric = ANY($5)
GROUP BY metric