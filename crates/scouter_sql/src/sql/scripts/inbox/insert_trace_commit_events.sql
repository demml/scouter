-- Bind: $1 = bytea[] (distinct trace_ids from the just-committed Delta batch)
INSERT INTO scouter.trace_commit_event (trace_id)
SELECT trace_id FROM unnest($1::bytea[]) AS t(trace_id);
