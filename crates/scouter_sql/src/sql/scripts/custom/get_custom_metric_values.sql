SELECT metric,
       AVG(value) AS value
FROM scouter.custom_drift
WHERE name = $1
  AND space = $2
  AND version = $3
  AND created_at > $4
  AND metric = ANY($5)
GROUP BY metric