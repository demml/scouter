# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Scouter** is a developer-first monitoring and observability toolkit for AAI workflows. It provides:
- **Drift detection** (PSI, SPC, custom metrics) for data and model monitoring
- **Distributed tracing** with OpenTelemetry-compatible span ingestion
- **GenAI evaluation** — both online (production sampling) and offline (batch)
- **Alerting** via Slack, OpsGenie, or Console, driven by scheduled background workers

The architecture is a Rust server + Python client. The Python package (`scouter-ml`) wraps Rust logic via PyO3. The server uses PostgreSQL for recent data and DataFusion for archival.

## Commands

### Rust (root)

```bash
cargo fmt --all                          # Format
cargo clippy --workspace --all-targets --all-features -- -D warnings  # Lint

make test.unit                           # Unit tests (types, dispatch, drift, profile) — no Docker needed
make test.needs_sql                      # SQL + server + eval + drift executor tests (requires Docker)
make test.sql                            # PostgreSQL integration tests only
make test.server                         # Server integration tests only
make test.drift.executor                 # Drift executor background worker tests

# Run a single Rust test
cargo test -p scouter-<crate> <test_name> --all-features -- --nocapture --test-threads=1

# Start local server (spins up all Docker backends first)
make start.server
```

### Python (`py-scouter/`)

```bash
make setup.project       # Build Rust extension (maturin) + sync Python deps — run after Rust changes
make format              # isort + black + ruff
make lints               # ruff + pylint + mypy

make test.unit           # pytest unit tests (excludes integration, benchmarks skipped)
make test.integration    # All integration tests (requires running server)
make test.integration.api    # FastAPI integration tests only
make test.integration.queue  # Queue integration tests only
make test.integration.client # Client integration tests only

# Run a single Python test
cd py-scouter && uv run pytest tests/path/to/test_file.py::test_name -s
```

### Docker

```bash
make build.all_backends  # Start Postgres + Kafka + Redis + RabbitMQ (needed for server tests)
make build.sql           # Start PostgreSQL only
make build.kafka         # Start Kafka + Zookeeper only
make build.shutdown      # Stop all services and remove volumes
```

## Architecture

### Repository Layout

```
scouter/
├── crates/              # Rust workspace crates
├── py-scouter/
│   ├── python/scouter/  # Python API (pure Python + thin wrappers over Rust types)
│   └── src/             # PyO3 FFI bindings (lib.rs registers all modules)
├── docker-compose.yml   # Local dev services
└── makefile             # Root build targets
```

### Key Rust Crates

| Crate | Purpose |
|-------|---------|
| `scouter-server` | HTTP (Axum) + gRPC (Tonic) server; API routes in `src/api/` |
| `scouter-sql` | PostgreSQL (sqlx), migrations, background workers (drift executor, GenAI poller) |
| `scouter-drift` | PSI, SPC, custom metric drift algorithms + binning strategies |
| `scouter-profile` | Data profiling (feature statistics, distributions) |
| `scouter-evaluate` | GenAI eval: LLM judge tasks, assertion tasks, comparison operators |
| `scouter-types` | Shared types/contracts across all crates |
| `scouter-events` | Kafka, RabbitMQ, Redis event bus adapters (feature-gated) |
| `scouter-tonic` | gRPC proto definitions |
| `scouter-tracing` | OpenTelemetry span ingestion and querying |
| `scouter-auth` | JWT authentication |
| `scouter-settings` | Server configuration (env vars → typed config) |
| `scouter-dispatch` | Alert dispatch (Slack, OpsGenie) |
| `scouter-observability` | Prometheus metrics setup |

### Python Package Structure (`py-scouter/python/scouter/`)

| Module | Key Classes |
|--------|-------------|
| `client/` | `ScouterClient` — register profiles, query drift results |
| `drift/` | `Drifter` — create `PsiDriftProfile`, `SpcDriftProfile`, `CustomMetricDriftProfile` |
| `profile/` | `DataProfiler` — create `DataProfile` with feature statistics |
| `evaluate/` | `GenAIEvalProfile`, `GenAIEvalDataset`, `GenAIEvalRecord` |
| `queue/` | `ScouterQueue` — real-time record insertion (<1µs, non-blocking) |
| `tracing/` | `init_tracer`, `get_tracer`, `TraceContext`, `SpanContext` |
| `genai/` | GenAI provider integrations (Anthropic, Google) |
| `transport/` | `HttpTransportConfig`, `KafkaTransportConfig`, `RabbitMQTransportConfig` |
| `alert/` | `SlackDispatchConfig`, `OpsGenieDispatchConfig`, `ConsoleDispatchConfig` |
| `types/` | `Features`, `Metrics`, `CustomMetric`, `AlertThreshold` |

PyO3 bindings in `py-scouter/src/lib.rs` register Rust modules into the Python extension. After changing Rust code, run `make setup.project` to rebuild. Stubs are generated via `make build.stubs`.

### Server Architecture

The server runs as a dual-protocol service with shared `Arc<AppState>`:
- **HTTP** (Axum): REST API for client requests
- **gRPC** (Tonic): High-performance transport (used by `ScouterQueue`)

`AppState` holds: PostgreSQL pool, task manager, auth manager, event consumer channels.

Background workers (in `scouter-sql`):
- **Drift Executor**: Scheduled via `pg_cron`; queries stored drift profiles, computes drift against recent data, triggers alerts
- **GenAI Poller**: Picks up pending `GenAIEvalRecord` batches, runs evaluation tasks, checks alert conditions

### Event Bus

Queue transport is feature-gated. The server binary is built with `--all-features` in CI/dev. Transport options: HTTP (default), Kafka (`--features kafka`), RabbitMQ (`--features rabbitmq`), Redis (`--features redis_events`).

## Domain Model

### Drift Profiles

Each profile type has a config + per-feature profile:

- **PSI (`PsiDriftProfile`)** — Bins baseline data into deciles; computes PSI = Σ(y_i - y_b) × ln(y_i / y_b). Thresholds: <0.1 stable, 0.1–0.25 moderate, >0.25 significant. Threshold types: `PsiFixedThreshold`, `PsiNormalThreshold`, `PsiChiSquareThreshold`.

- **SPC (`SpcDriftProfile`)** — Calculates grand mean + stddev from baseline; establishes 3 control limit zones (1σ, 2σ, 3σ). Uses WECO rules (default "8 16 4 8 2 4 1 1") to detect out-of-control patterns.

- **Custom (`CustomMetricDriftProfile`)** — User-defined named metrics with `AlertThreshold` (Below, Above, Outside). `sample_size` (default 25) controls aggregation before threshold check.

### GenAI Evaluation

Two evaluation modes:

**Online** (`GenAIEvalProfile`): Real-time monitoring. `GenAIEvalRecord` objects are inserted into `ScouterQueue` during inference. Server samples based on `sample_ratio`, runs evaluation tasks asynchronously, checks alert conditions on schedule.

**Offline** (`GenAIEvalDataset`): Batch evaluation. Records → Tasks → Dataset → Execute → Review results.

**Task types:**
- `AssertionTask` — Deterministic checks using 50+ `ComparisonOperator` values (Equals, GreaterThan, Contains, Matches, IsJson, etc.). Supports `context_path` (dot-notation field extraction) and template variable substitution (`${field_name}`). Set `condition=True` to act as a conditional gate (skips downstream tasks on failure).
- `LLMJudgeTask` — LLM-powered semantic evaluation. Injects context variables into `Prompt`. Supports OpenAI, Anthropic, Google. Uses structured output (Pydantic models). `context_path` extracts field from LLM response.

Tasks can declare `depends_on: ["task_id"]` to access upstream outputs. Each task sees base context + declared dependencies only.

### Alert Dispatch

Alert configs attach to drift/eval profiles and run on a cron schedule:
- `SlackDispatchConfig(channel, token)`
- `OpsGenieDispatchConfig(team, api_key)`
- `ConsoleDispatchConfig`
- `CommonCrons` enum: `EveryHour`, `Every6Hours`, `EveryDay`, etc.

### Data Types

Scouter accepts: Pandas DataFrames, NumPy 2D arrays, Polars DataFrames, Pydantic models.

## Key Conventions

- **Rust errors**: `thiserror` for error types; propagate with `?`
- **Async**: Tokio multi-threaded throughout
- **Python deps**: Managed with `uv`; use `uv run` for all Python commands in `py-scouter/`
- **SQL tests**: Must use `--test-threads=1` for isolation
- **Clippy**: `-D warnings` — all warnings are errors in CI
- **Profile versioning**: Profiles are identified by `(space, name, version)` triple — used throughout the SQL schema and API routing
- **Feature gates**: New event bus integrations go in `scouter-events` with a Cargo feature flag

## Server Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URI` | required | PostgreSQL connection string |
| `MAX_POOL_SIZE` | — | DB connection pool size |
| `DATA_RETENTION_PERIOD` | 30 days | Auto-purge threshold |
| `POLLING_WORKER_COUNT` | 4 | Background job workers |
| `KAFKA_BROKERS` | — | Kafka bootstrap servers |
| `RABBITMQ_ADDR` | — | RabbitMQ AMQP URL |
| `REDIS_ADDR` | — | Redis URL |
| `SCOUTER_STORAGE_URI` | `./scouter_storage` | Object storage (S3, GCS, Azure, local) |
| `SCOUTER_ENCRYPT_SECRET` | — | HMAC-SHA256 key (32 bytes) |
| `SCOUTER_BOOTSTRAP_KEY` | — | Initial admin bootstrap key |
