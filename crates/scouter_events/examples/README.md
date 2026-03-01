# scouter-events Integration Examples

End-to-end integration tests for the three event bus transports: Kafka, RabbitMQ, and Redis. Each example starts a background producer, sends records through the event bus, and verifies that they accumulate in PostgreSQL.

These are integration tests, not tutorials — they require all backends running and are executed via makefile targets.

## Prerequisites

- Docker + Docker Compose
- All backends running: `make build.all_backends` from the repo root

## Running

```bash
# From repo root

make test.kafka_events
make test.rabbitmq_events
make test.redis_events
```

Each target calls `cargo run --example <name> --all-features`.

## Examples

### `kafka_integration.rs`

Verifies that SPC drift records published to a Kafka topic are consumed by the server and persisted to PostgreSQL.

**What it does:**
1. Creates a drift profile entity in the database via `TestHelper`
2. Spawns a background task that publishes `SpcRecord` batches to the `scouter_monitoring` Kafka topic every 15 seconds
3. Waits for a warming period, then asserts that 5,000+ records have accumulated in the database

**Requires:** `KAFKA_BROKERS` env var (defaults to `localhost:9092`). Kafka transport must be compiled in — the makefile passes `--all-features`.

---

### `rabbitmq_integration.rs`

Same flow as the Kafka example, using RabbitMQ as the transport.

**What it does:**
1. Creates a drift profile entity in the database
2. Spawns a background producer that serializes `ServerRecords` and publishes them to the `scouter_monitoring` RabbitMQ queue using `lapin`
3. Waits, then asserts 5,000+ records in PostgreSQL

**Requires:** `RABBITMQ_ADDR` env var (defaults to `amqp://guest:guest@127.0.0.1:5672/%2f`).

---

### `redis_integration.rs`

Same flow using Redis Pub/Sub.

**What it does:**
1. Creates a drift profile entity in the database
2. Spawns a background producer using `RedisProducer` to publish `MessageRecord::ServerRecords` to the `scouter_monitoring` channel
3. Waits, then asserts 5,000+ records in PostgreSQL

**Requires:** `REDIS_ADDR` env var (defaults to `redis://127.0.0.1:6379`). Compiled with `redis_events` feature via `--all-features`.

---

### `utils.rs`

Shared test infrastructure used by all three examples. Not an example itself.

**Provides:**
- `setup_logging()` — structured JSON logging via `tracing_subscriber`
- `TestHelper` — loads `ScouterServerConfig`, creates a PostgreSQL pool, and cleans up test data on init
- `cleanup(pool)` — truncates drift and monitoring tables between runs

## Feature flags

The event bus integrations are feature-gated. They are only compiled when the corresponding feature is enabled:

| Transport | Feature flag |
|-----------|-------------|
| Kafka | `kafka` |
| RabbitMQ | `rabbitmq` |
| Redis | `redis_events` |

The makefile passes `--all-features` so all transports compile during development. In production builds, only enable the features you need.
