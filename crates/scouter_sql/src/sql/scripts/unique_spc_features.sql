SELECT
DISTINCT feature
FROM scouter.drift
WHERE
   1=1
   AND name = $1
   AND space = $2
   AND version = $3;