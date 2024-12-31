SELECT
DISTINCT feature
FROM scouter.drift
WHERE
   name = $1
   AND repository = $2
   AND version = $3;