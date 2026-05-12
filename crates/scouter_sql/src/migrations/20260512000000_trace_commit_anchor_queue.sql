-- Forward-only migration: rebuild scouter.trace_commit_event from a trace-level
-- inbox into an anchor-level durable queue / audit table.
--
-- The 20260509000000 migration shipped a trace-level inbox keyed only on trace_id.
-- That schema cannot represent the per-span anchor contract this change requires.
-- Deploy this migration during downtime with the server stopped.

DROP TABLE IF EXISTS scouter.trace_commit_event;

CREATE TABLE scouter.trace_commit_event (
    id             BIGSERIAL PRIMARY KEY,

    trace_id       BYTEA NOT NULL CHECK (octet_length(trace_id) = 16),
    span_id        BYTEA NOT NULL CHECK (octet_length(span_id) = 8),
    record_uid     TEXT  NOT NULL,
    profile_uid    TEXT  NOT NULL,

    status         TEXT        NOT NULL DEFAULT 'pending',
    attempt_count  INT         NOT NULL DEFAULT 0,
    claimed_at     TIMESTAMPTZ,
    claimed_by     TEXT,
    claim_token    UUID,
    processed_at   TIMESTAMPTZ,
    last_error     TEXT,

    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT trace_commit_event_record_span_unique UNIQUE (record_uid, span_id),
    CONSTRAINT chk_trace_commit_event_record_uid
        CHECK (
            char_length(record_uid) BETWEEN 1 AND 128
            AND record_uid ~ '^[A-Za-z0-9_.:-]+$'
        ),
    CONSTRAINT chk_trace_commit_event_profile_uid
        CHECK (
            char_length(profile_uid) BETWEEN 1 AND 128
            AND profile_uid ~ '^[A-Za-z0-9_.:-]+$'
        ),
    CONSTRAINT chk_trace_commit_event_status
        CHECK (status IN ('pending', 'processing', 'processed', 'dead_lettered'))
);

DROP INDEX IF EXISTS scouter.idx_agent_eval_record_awaiting_trace;

CREATE INDEX IF NOT EXISTS idx_agent_eval_record_awaiting_trace_uid
    ON scouter.agent_eval_record (uid)
    WHERE status = 'awaiting_trace';

CREATE INDEX IF NOT EXISTS idx_agent_eval_record_awaiting_trace_reconcile
    ON scouter.agent_eval_record (created_at, uid)
    WHERE status = 'awaiting_trace'
      AND trace_id IS NOT NULL
      AND span_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_agent_eval_record_awaiting_trace_timeout
    ON scouter.agent_eval_record (created_at)
    WHERE status = 'awaiting_trace';

CREATE INDEX IF NOT EXISTS idx_trace_commit_event_pending
    ON scouter.trace_commit_event (id)
    WHERE status = 'pending';

CREATE INDEX IF NOT EXISTS idx_trace_commit_event_record_uid
    ON scouter.trace_commit_event (record_uid);

CREATE INDEX IF NOT EXISTS idx_trace_commit_event_processing
    ON scouter.trace_commit_event (claimed_at)
    WHERE status = 'processing';

CREATE INDEX IF NOT EXISTS idx_trace_commit_event_processed_at
    ON scouter.trace_commit_event (processed_at)
    WHERE status = 'processed';

CREATE INDEX IF NOT EXISTS idx_trace_commit_event_dead_lettered
    ON scouter.trace_commit_event (created_at)
    WHERE status = 'dead_lettered';
