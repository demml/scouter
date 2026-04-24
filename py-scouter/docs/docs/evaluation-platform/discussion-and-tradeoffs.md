# Trade-offs and operational considerations

This section covers the things you need to know before running Scouter in production. The previous sections describe how the system works; this one describes where it gets interesting under load, what it costs, and where the sharp edges are.

For system-wide architecture constraints, see [Architecture](../architecture/overview.md). For the regression comparison API, see [Comparing runs](../agents/offline-evaluation.md#saving-loading-and-comparing-results).

---

## Deployment model

Scouter runs as a stateless container (Rust binary) backed by PostgreSQL and an object store (S3, GCS, Azure, or local disk). Configuration is environment variables. No local state persists between restarts except Delta Lake data on shared object storage.

### Worker pool sizing

The server runs six independent worker pools. Each scales separately.

| Pool | Env var | Default | What it does |
|---|---|---|---|
| Server record ingestion | `SERVER_RECORD_CONSUMER_WORKERS` | 4 | Processes drift profiles and eval records from the flume channel (bounded at 1000) |
| Trace ingestion | `TRACE_CONSUMER_WORKERS` | 2 | Processes OTLP span batches from the trace channel (bounded at 500) |
| Tag ingestion | `TAG_CONSUMER_WORKERS` | 1 | Processes tag records from the tag channel (bounded at 200) |
| Drift executor | `POLLING_WORKER_COUNT` | 4 | Polls PostgreSQL for pending drift computation jobs |
| Agent eval | `GENAI_WORKER_COUNT` | 2 | Polls PostgreSQL for pending evaluation records |
| Trace eval | `TRACE_EVAL_WORKER_COUNT` | 1 (forced) | Scans dispatch table for trace-to-eval candidates |

The three ingestion channels are independent. Trace volume doesn't starve drift record processing; they have separate channels, separate workers, and separate backpressure. Size each based on its own throughput.

The polling workers (drift, agent eval, trace eval) each hold a PostgreSQL connection during their poll-execute-store cycle. With defaults, that's 4 + 2 + 1 = 7 connections dedicated to background work. Factor this into your `MAX_POOL_SIZE` alongside API request connections.

All workers start with 200ms stagger to avoid thundering herd at boot.

### Delta Lake concurrency model

Each pod runs a single engine actor per Delta table, serializing local writes through an mpsc channel. Across pods, Delta Lake's optimistic concurrency control handles concurrent appends to shared object storage. Multiple pods can ingest spans simultaneously without coordination.

The constraint is compaction and vacuum, not writes. Compaction is coordinated via `ControlTableEngine` so only one pod compacts at a time. Post-compaction vacuum runs with `retention_hours=0`, which relies on all pods refreshing their Delta snapshot frequently enough that no pod references stale files. At the default 10-second refresh interval (`SCOUTER_TRACE_REFRESH_INTERVAL_SECS`), this works. If you set the refresh interval very high (minutes), increase the vacuum retention to match.

### Shutdown sequence

Shutdown happens in order:

1. Broadcast shutdown signal to all workers
2. 5-second drain window for in-flight work to complete
3. Force-abort remaining tasks
4. Flush trace cache to PostgreSQL
5. Signal Delta Lake services to flush buffers and shut down (trace spans, summaries, dispatch, GenAI spans)
6. Shut down dataset manager
7. Close database pool

The 5-second drain window covers all but pathologically slow database inserts. If your evaluation tasks include `LLMJudgeTask` calls that take longer than 5 seconds, those will be aborted and the record will remain in `pending` status for the next startup to pick up.

---

## Latency characteristics

### Client-side

Sub-microsecond queue insertion. The GIL is released during the push to the crossbeam `ArrayQueue`. Your application doesn't notice.

### Server-side without trace assertions

Record arrives via transport, enters the flume channel, gets written to PostgreSQL. The `AgentPoller` picks it up on its next poll cycle (up to 1 second sleep between polls when idle). Task execution depends on what's in the profile: sub-second for assertion-only profiles, seconds for profiles with `LLMJudgeTask` calls. Results go back to PostgreSQL.

End-to-end: low single-digit seconds for assertion-only. Add the LLM provider's latency for judge tasks.

### Server-side with trace assertions

Trace assertions need spans, and spans often arrive after the eval record. The poller waits with exponential backoff (100ms, 200ms, 400ms, up to 5-second cap per attempt). If spans aren't available within the timeout (default 10 seconds), the record is rescheduled 30 seconds out. After 3 failed attempts, the record is marked `failed`.

Worst case at default settings: ~90 seconds from record insertion to stored result. If your span arrival p95 is under 5 seconds, tighten these values:

| Variable | Default | Tuning guidance |
|---|---|---|
| `GENAI_TRACE_WAIT_TIMEOUT_SECS` | 10 | Lower if spans arrive fast. 5 seconds works for most Delta Lake flush intervals. |
| `GENAI_TRACE_BACKOFF_MILLIS` | 100 | Initial backoff between span checks. Raise if you want fewer database queries. |
| `GENAI_TRACE_RESCHEDULE_DELAY_SECS` | 30 | How far forward to push a failed attempt. Lower for faster retry, higher to reduce churn. |
| `GENAI_MAX_RETRIES` | 3 | Total rescheduled attempts before marking as failed. |

### Alert latency

Alerts run on a cron schedule via the `DriftExecutor`. Minimum alert latency equals your cron interval. `EveryHour` means up to an hour before an alert fires after quality degrades. For faster response, use a tighter schedule (the `CommonCrons` enum includes `Every6Hours`, `Every12Hours`, `EveryDay`, `EveryHour`, or pass a custom cron expression).

The trade-off: tighter schedules mean more frequent aggregation queries against PostgreSQL. For most deployments, hourly is the right starting point. Drop to custom 15-minute intervals only if you have high traffic and need fast alerting.

---

## Cost model

### Infrastructure

You run it. The costs are:

- PostgreSQL instance (drift records, eval records, results, profiles, alert config)
- Object storage (Delta Lake tables for trace spans and GenAI spans)
- Compute (Scouter server binary, moderate memory footprint)

No per-eval, per-span, or per-trace pricing. Assertion-only evaluation profiles cost nothing beyond the compute you already run. SaaS alternatives charge per-span or per-trace, which adds up at high volume.

### LLM judge costs

Each `LLMJudgeTask` in an online profile costs one LLM API call per sampled record. Total cost:

```
traffic_volume × sample_ratio × judge_tasks_per_profile × cost_per_call
```

Conditional gates reduce this directly. If a cheap `AssertionTask` gate fails, the `LLMJudgeTask` never runs and you save the API call. We'd recommend starting with `sample_ratio=0.1`, one judge task behind a gate, and tuning based on signal quality. At 10% sampling with one gated judge, most records never trigger an LLM call.

### Comparison to SaaS

| Model | Scaling behavior |
|---|---|
| Datadog LLM Obs | Per-span pricing. Cost scales linearly with trace volume. |
| LangSmith | Per-trace pricing. Evaluation cost scales with traffic. |
| Scouter | Fixed infrastructure. Evaluation cost scales only with LLM judge calls. Assertion-only profiles are free to run. |

For high-volume services (millions of spans/day), the fixed-cost model is significantly cheaper. For low-volume prototyping, the SaaS convenience may be worth the per-unit cost.

---

## SKIP LOCKED is not a durable job queue

The polling model uses PostgreSQL's `SELECT ... FOR UPDATE SKIP LOCKED` for work distribution. This works well for the common case but has a gap worth knowing about.

The window between locking a record and updating its status is not a single transaction in all paths. The `DriftExecutor` advances run dates unconditionally after processing a drift task, regardless of whether the computation succeeded or failed. If drift computation fails (transient database error, LLM provider timeout), the schedule still advances. The next scheduled run skips the failure window's data.

For agent evaluation, the pattern is tighter: the `AgentPoller` updates the record status to `completed` or `failed` within the same flow, and failed records with retries remaining get rescheduled rather than lost.

If a worker crashes mid-computation (OOM, pod eviction), the locked record becomes visible again after the transaction times out. Double-computation is possible at the crash boundary. For most workloads, this is fine because evaluation is idempotent. For workloads where every evaluation window matters, monitor the `agent_eval_record` table's `failed` count and set up alerting.

---

## Evaluation-as-CI: integration patterns

### Pattern 1: PR-gated evaluation

1. Maintain a scenario suite in source control (JSON, YAML, or programmatic)
2. CI runs `EvalOrchestrator` on every PR against the scenario suite
3. Compare results against a stored baseline via `ScenarioEvalResults.compare_to(baseline, regression_threshold=0.05)`
4. Block merge if any alias regresses beyond the threshold
5. On merge to main, save the new baseline as a CI artifact

This catches regressions in prompt changes, tool definitions, and orchestration logic before they hit production. The threshold (default 5% pass-rate drop) is configurable per deployment. Start strict and relax if flaky tests become a problem.

### Pattern 2: scheduled regression monitoring

1. Nightly CI job runs the full scenario suite against the deployed agent
2. Compare against the last known-good baseline
3. Alert on regression (Slack, OpsGenie, or CI notification)

This catches silent provider-side degradation: model version changes, API behavior changes, rate limit adjustments. Things that don't show up in code diffs but degrade quality.

### Pattern 3: canary evaluation

1. Deploy a new agent version to canary
2. Online evaluation runs on canary traffic with a tighter `sample_ratio`
3. Compare canary pass rates against production baseline
4. Promote or rollback based on eval results

This bridges offline and online evaluation. Offline tests catch known regressions; canary evaluation catches regressions that only surface with real traffic distribution.

### Scenario suite evolution

Start with scenarios covering the happy path. Add a scenario for every production failure (post-mortem becomes test case). Use `expected_outcome` and template variables (`${ground_truth.answer}`) for ground truth comparison. Version scenario files alongside code.

A good scenario suite grows monotonically. Don't remove scenarios that pass; they're regression guards. Only remove scenarios that test behavior you've intentionally changed.

---

## Extensibility

Scouter is designed to be extended at specific points:

- **Comparison operators.** Add to the `ComparisonOperator` enum in `scouter_types`, implement the comparison logic in `AssertionEvaluator`. Both Rust changes.
- **Task types.** Add to the task type enum, implement evaluation logic in `scouter_evaluate`, add PyO3 bindings. Follow the pattern of existing task types.
- **Transport backends.** Feature-gated in `scouter-events`. Implement the producer/consumer traits. Current gates: `kafka`, `rabbitmq`, `redis_events`.
- **Alert dispatch channels.** Implement the dispatch trait in `scouter-dispatch`. Current implementations: Slack, OpsGenie, Console.
- **Custom orchestrators.** Subclass `EvalOrchestrator` in Python for framework-specific agent execution. Override `execute_agent()`, `on_scenario_start()`, `on_scenario_complete()`, `build_scenario_response()`. All hooks are no-ops by default.

---

## Known constraints

These are documented here and in the [architecture overview](../architecture/overview.md). Linking for completeness:

- `EvalRunner.evaluate()` calls `block_on` internally. It checks for an existing async runtime and returns an error if one is detected. You cannot call it from inside an async context. Use `evaluate_async()` from async code, or call `evaluate()` from synchronous Python.
- Queue insert errors are logged at ERROR level, not returned to the caller. No counter is exported by default. Monitor server logs for `Error inserting entity into queue`.
- The `DriftExecutor` advances run dates unconditionally, even if drift computation fails. A failed run still moves the schedule forward.
- PSI flushes every 30 seconds; SPC does not. Low-traffic SPC services accumulate records longer than expected before triggering evaluation.
- Span hierarchy (depth, span_order, path) is computed at query time via Rust DFS. Deep traces with hundreds of spans are slower to assemble.
- Delta Lake writes scale horizontally (OCC handles concurrent appends), but compaction and vacuum are single-pod operations coordinated via the control table. Aggressive vacuum (`retention_hours=0`) relies on all pods refreshing their snapshot within the refresh interval.
- The class that checks agent eval alerts inside the `DriftExecutor` is called `AgentDrifter`. This is a naming artifact, not an architectural relationship. The `AgentPoller` (task evaluator) and `AgentDrifter` (alert checker) are completely separate workers with no shared code or state.
