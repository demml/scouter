select
from scouter.genai_eval_task
where record_uid = $1
ORDER BY STAGE ASC, START_TIME ASC;