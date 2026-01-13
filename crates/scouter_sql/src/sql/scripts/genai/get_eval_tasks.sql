select *
from genai_eval_tasks
where record_uid = $1
ORDER BY STAGE ASC, START_TIME ASC;