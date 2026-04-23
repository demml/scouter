SELECT encode(trace_id, 'hex') AS trace_id_hex
FROM scouter.agent_eval_record
WHERE entity_id = $1
  AND trace_id IS NOT NULL
  AND created_at > $2;
