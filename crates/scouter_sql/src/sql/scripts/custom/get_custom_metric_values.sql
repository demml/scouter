SELECT metric,
       AVG(value) AS value
FROM scouter.custom_metric
WHERE name = $1
  AND space = $2
  AND version = $3
  AND created_at > $4::timestamp
  AND metric = ANY($5)
GROUP BY metric