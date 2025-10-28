SELECT
  id,
  created_at, 
  uid,
  space, 
  name, 
  version, 
  prompt, 
  inputs,
  outputs, 
  ground_truth,
  metadata, 
  entity_type,
  root_id,
  event_id,
  event_name,
  parent_event_name,
  duration_ms,
  status, 
  updated_at,
  processing_started_at,
  processing_ended_at,
  processing_duration
FROM scouter.llm_event_record
WHERE 1=1
  AND space = $1
  AND name = $2
  AND version = $3