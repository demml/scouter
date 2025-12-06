SELECT
DISTINCT feature
FROM scouter.spc_drift
WHERE
   1=1
   AND entity_id = $1;