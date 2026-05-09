-- Add span_id to agent_eval_record (nullable; only set on Path A inserts)
ALTER TABLE scouter.agent_eval_record
    ADD COLUMN span_id BYTEA CHECK (span_id IS NULL OR octet_length(span_id) = 8);

-- Trace-backed eval rows become poller-eligible only after local Delta snapshots
-- have had time to observe the post-commit trace. This is separate from
-- scheduled_at, which remains the retry/reschedule clock.
ALTER TABLE scouter.agent_eval_record
    ADD COLUMN ready_at TIMESTAMPTZ NOT NULL DEFAULT now();

-- Partial index supporting the inbox flip — find awaiting_trace rows for a set of trace_ids
CREATE INDEX IF NOT EXISTS idx_agent_eval_record_awaiting_trace
    ON scouter.agent_eval_record (trace_id)
    WHERE status = 'awaiting_trace' AND trace_id IS NOT NULL;

-- Poller hot path: pending rows must be both scheduled and ready.
CREATE INDEX IF NOT EXISTS idx_agent_eval_record_ready_polling
    ON scouter.agent_eval_record(status, ready_at, scheduled_at, retry_count)
    WHERE status = 'pending';

-- Transactional inbox: durable post-commit events from trace ingest
CREATE TABLE IF NOT EXISTS scouter.trace_commit_event (
    id            BIGSERIAL PRIMARY KEY,
    trace_id      BYTEA NOT NULL CHECK (octet_length(trace_id) = 16),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    processed_at  TIMESTAMPTZ,
    error         TEXT
);

-- Hot-path index for the event worker's claim query (oldest-unprocessed-first)
CREATE INDEX IF NOT EXISTS idx_trace_commit_event_unprocessed
    ON scouter.trace_commit_event (id)
    WHERE processed_at IS NULL;

-- Supports the reverse-race short-circuit: insert path probes
-- `EXISTS(SELECT 1 FROM trace_commit_event WHERE trace_id = $1)` to detect
-- "trace already committed" and route the eval row directly to `pending`.
-- Covers both processed and unprocessed events (1d retention is the cache TTL).
CREATE INDEX IF NOT EXISTS idx_trace_commit_event_trace_id
    ON scouter.trace_commit_event (trace_id);

-- Supports the prune sweep
CREATE INDEX IF NOT EXISTS idx_trace_commit_event_processed_at
    ON scouter.trace_commit_event (processed_at)
    WHERE processed_at IS NOT NULL;
