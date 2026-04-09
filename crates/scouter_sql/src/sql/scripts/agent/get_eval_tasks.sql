select *
from scouter.agent_eval_task
where record_uid = $1
ORDER BY STAGE ASC, START_TIME ASC;