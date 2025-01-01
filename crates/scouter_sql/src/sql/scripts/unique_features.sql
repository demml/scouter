SELECT
DISTINCT feature
FROM drift
WHERE
   name = $1
   AND repository = $2
   AND version = $3;