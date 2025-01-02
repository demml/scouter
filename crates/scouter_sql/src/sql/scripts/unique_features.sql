SELECT
DISTINCT feature
FROM drift
WHERE
   1=1
   AND name = $1
   AND repository = $2
   AND version = $3;