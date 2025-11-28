SELECT metric,
       AVG(value) AS value
FROM scouter.custom_drift
WHERE 1=1
  AND created_at > $4
  AND entity_id = $3
  AND metric = ANY($4)
GROUP BY metric