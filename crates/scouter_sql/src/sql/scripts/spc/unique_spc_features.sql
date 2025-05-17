SELECT
DISTINCT feature
FROM scouter.spc_drift
WHERE
   1=1
   AND space = $2
   AND name = $1
   AND version = $3;