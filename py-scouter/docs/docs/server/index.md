The Scouter server is a Rust-based server designed to run independent of the Python client. It handles the event system (via Kafka, RabbitMQ, Redis, or the default HTTP queue), database CRUD operations, background drift detection, Agent evaluation, and alerting.

Features:

- <span class="text-secondary">**Event System**</span> - Supports Kafka, RabbitMQ, Redis, and a built-in HTTP queue for sending monitoring events to the server
- <span class="text-secondary">**Database Storage**</span> - Leverages **Postgres** (sqlx) for short-term data storage and **DataFusion** (Parquet/object store) for long-term archival
- <span class="text-secondary">**Alerting**</span> - Integrates with OpsGenie and Slack for alerting
- <span class="text-secondary">**Data Retention and Partitioning**</span> - Built-in retention and partitioning strategies via `pg_partman` and `pg_cron`
- <span class="text-secondary">**Authentication**</span> - Built-in JWT authentication system
- <span class="text-secondary">**gRPC + HTTP**</span> - Dual-protocol server (Axum HTTP on port 8000, Tonic gRPC on port 50051)

---

## Getting Started Locally

This section is for **developers** who want to run Scouter locally for development or testing.

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable toolchain)
- [Docker](https://docs.docker.com/get-docker/) + Docker Compose
- [uv](https://docs.astral.sh/uv/) (for Python client work)
- Git

### 1. Clone the repository

```bash
git clone https://github.com/demml/scouter.git
cd scouter
```

### 2. Start the backend services

This spins up PostgreSQL, Kafka, RabbitMQ, and Redis via Docker Compose:

```bash
make build.all_backends
```

This uses the `server-backends` Docker Compose profile which starts all services and waits until they are healthy before returning.

### 3. Start the server

```bash
make start.server
```

This will:

1. Kill any existing process on port 8000
2. Start all backend services (if not already running)
3. Build the server binary with all event bus features enabled
4. Start the server in the background with Kafka, RabbitMQ, and Redis configured

The server will be available at `http://localhost:8000`.

### 4. Verify the server is running

```bash
curl http://localhost:8000/healthcheck
```

### 5. (Optional) Set up the Python client

After making any Rust changes, rebuild the Python extension:

```bash
cd py-scouter
make setup.project
```

Then configure the client to point at your local server:

```bash
export SCOUTER_SERVER_URI=http://localhost:8000
export SCOUTER_USERNAME=admin
export SCOUTER_PASSWORD=admin
```

### 6. Shut down all services

```bash
make build.shutdown
```

### Running tests locally

```bash
# Unit tests — no Docker required
make test.unit

# Integration tests — requires backends running
make build.all_backends
make test.needs_sql

# Python unit tests
cd py-scouter && make test.unit

# Python integration tests — requires running server
cd py-scouter && make test.integration
```

---

## Getting Started (Production)

There are a few different ways to deploy the Scouter server in production.

### Prerequisites

Scouter requires a **PostgreSQL 16.3+** database with the `pg_partman` and `pg_cron` extensions. See the [PostgreSQL setup guide](postgres.md) for details.

Set the following environment variable before starting the server:

```bash
export DATABASE_URI=postgresql://<user>:<password>@<host>:<port>/<db>
```

### Docker

Pre-built Docker images are published on every release for the following platforms:

<span class="text-primary">**Amd64** (x86_64-unknown-linux-gnu)</span>

| Image | Tag Suffix | Features |
|-------|------------|---------|
| ubuntu | ubuntu | Kafka, RabbitMQ |
| alpine | alpine | Kafka, RabbitMQ |
| scratch | scratch | Kafka, RabbitMQ |
| debian | debian | Kafka, RabbitMQ |
| distroless | distroless | Kafka, RabbitMQ |

<span class="text-primary">**Arm64** (aarch64-unknown-linux-gnu)</span>

| Image | Tag Suffix | Features |
|-------|------------|---------|
| ubuntu | ubuntu | Kafka, RabbitMQ |
| alpine | alpine | Kafka, RabbitMQ |
| scratch | scratch | Kafka, RabbitMQ |
| debian | debian | Kafka, RabbitMQ |
| distroless | distroless | Kafka, RabbitMQ |

#### Pull your image

```bash
docker pull demml/scouter:ubuntu-amd64-kafka-latest
```

#### Run your image

```bash
docker run -d \
  --name scouter \
  -p 8000:8000 \
  -e DATABASE_URI=postgresql://user:pass@host:5432/db \
  -e SCOUTER_ENCRYPT_SECRET=<your-32-byte-secret> \
  -e SCOUTER_REFRESH_SECRET=<your-32-byte-secret> \
  -e SCOUTER_BOOTSTRAP_KEY=<your-32-byte-key> \
  demml/scouter:ubuntu-amd64-kafka-latest
```

### Pre-built binaries

Binaries for various architectures are published on every release. Download and run the binary directly:

```bash
# Download from GitHub releases
curl -L https://github.com/demml/scouter/releases/latest/download/scouter-server-linux-amd64 -o scouter-server
chmod +x scouter-server
./scouter-server
```

Binaries can be found [here](https://github.com/demml/scouter/releases).

### Build from source

```bash
# Default build (HTTP queue only)
cargo build -p scouter-server --release

# With Kafka support
cargo build -p scouter-server --release --features "kafka"

# With RabbitMQ support
cargo build -p scouter-server --release --features "rabbitmq"

# With Redis support
cargo build -p scouter-server --release --features "redis_events"

# With all event bus features
cargo build -p scouter-server --release --all-features
```

### Feature Flags

| Flag | Description |
|------|-------------|
| `kafka` | Enables Kafka consumer via the `rdkafka` crate |
| `rabbitmq` | Enables RabbitMQ consumer via the `lapin` crate |
| `redis_events` | Enables Redis Pub/Sub consumer |

---

## Environment Variables

### Database Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `DATABASE_URI` | PostgreSQL connection string | `postgresql://postgres:postgres@localhost:5432/postgres` |
| `MAX_POOL_SIZE` | Maximum DB connections in the pool | `200` |
| `MIN_POOL_SIZE` | Minimum DB connections in the pool | `20` |
| `DB_ACQUIRE_TIMEOUT_SECONDS` | Timeout (seconds) to acquire a connection | `10` |
| `DB_IDLE_TIMEOUT_SECONDS` | Idle connection timeout (seconds) | `300` |
| `DB_MAX_LIFETIME_SECONDS` | Maximum connection lifetime (seconds) | `1800` |
| `DB_TEST_BEFORE_ACQUIRE` | Test connections before acquiring from pool | `true` |
| `DATA_RETENTION_PERIOD` | Days to retain data in the DB before archiving to long-term storage | `30` |
| `TRACE_FLUSH_INTERVAL_SECONDS` | How often (seconds) to flush trace spans to the DB | `15` |
| `TRACE_STALE_THRESHOLD_SECONDS` | How long (seconds) before an open trace span is considered stale | `30` |
| `TRACE_CACHE_MAX_SIZE` | Maximum number of trace spans to hold in memory | `10000` |
| `ENTITY_CACHE_MAX_SIZE` | Maximum number of entity definitions to cache in memory | `1000` |

### Polling Variables

Background workers that run scheduled drift detection and alerting.

| Variable | Description | Default |
|----------|-------------|---------|
| `POLLING_WORKER_COUNT` | Number of drift detection/alerting worker threads | `4` |
| `MAX_RETRIES` | Maximum retries for failed polling tasks | `3` |

### Agent Polling Variables

Background workers for asynchronous Agent evaluation.

| Variable | Description | Default |
|----------|-------------|---------|
| `GENAI_WORKER_COUNT` | Number of agent evaluation worker threads | `2` |
| `GENAI_MAX_RETRIES` | Maximum retries for failed agent evaluation tasks | `3` |
| `GENAI_TRACE_WAIT_TIMEOUT_SECS` | Seconds to wait for a trace to arrive before timing out | `10` |
| `GENAI_TRACE_BACKOFF_MILLIS` | Backoff delay (ms) between trace polling attempts | `100` |
| `GENAI_TRACE_RESCHEDULE_DELAY_SECS` | Delay (seconds) before rescheduling a failed agent evaluation task | `30` |

### HTTP Queue Variables

For the built-in HTTP event consumer (default transport, no extra setup required).

| Variable | Description | Default |
|----------|-------------|---------|
| `HTTP_CONSUMER_WORKER_COUNT` | Number of HTTP consumer worker threads | `1` |

### Kafka Variables

Enabled when `KAFKA_BROKERS` is set and the `kafka` feature flag is compiled in.

| Variable | Description | Default |
|----------|-------------|---------|
| `KAFKA_BROKERS` | Comma-separated list of Kafka broker addresses | `localhost:9092` |
| `KAFKA_WORKER_COUNT` | Number of Kafka consumer worker threads | `3` |
| `KAFKA_TOPIC` | Kafka topic(s) to consume (comma-separated) | `scouter_monitoring` |
| `KAFKA_GROUP` | Kafka consumer group ID | `scouter` |
| `KAFKA_OFFSET_RESET` | Offset reset policy | `earliest` |
| `KAFKA_USERNAME` | SASL username | — |
| `KAFKA_PASSWORD` | SASL password | — |
| `KAFKA_SECURITY_PROTOCOL` | Security protocol (`PLAINTEXT`, `SSL`, `SASL_PLAINTEXT`, `SASL_SSL`) | `SASL_SSL` |
| `KAFKA_SASL_MECHANISM` | SASL mechanism | `PLAIN` |
| `KAFKA_CERT_LOCATION` | Path to CA certificate file | — |

### RabbitMQ Variables

Enabled when `RABBITMQ_ADDR` is set and the `rabbitmq` feature flag is compiled in.

| Variable | Description | Default |
|----------|-------------|---------|
| `RABBITMQ_ADDR` | RabbitMQ AMQP address | `amqp://guest:guest@127.0.0.1:5672/%2f` |
| `RABBITMQ_CONSUMER_COUNT` | Number of RabbitMQ consumers | `3` |
| `RABBITMQ_PREFETCH_COUNT` | Messages to prefetch per consumer | `10` |
| `RABBITMQ_QUEUE` | Queue name | `scouter_monitoring` |
| `RABBITMQ_CONSUMER_TAG` | Consumer tag | `scouter` |

### Redis Variables

Enabled when `REDIS_ADDR` is set and the `redis_events` feature flag is compiled in.

| Variable | Description | Default |
|----------|-------------|---------|
| `REDIS_ADDR` | Redis server address | `redis://127.0.0.1:6379` |
| `REDIS_CONSUMER_COUNT` | Number of Redis consumer workers | `3` |
| `REDIS_CHANNEL` | Redis Pub/Sub channel name | `scouter_monitoring` |

### Authentication

Scouter uses JWT-based authentication. For production deployments, always set all three secrets to strong random values — if any are unset, Scouter will fall back to a **deterministic default key** which is unsafe.

| Variable | Description |
|----------|-------------|
| `SCOUTER_ENCRYPT_SECRET` | Signs JWT access tokens. Must be a base64-encoded PBKDF2-HMAC-SHA256 key (32 bytes). |
| `SCOUTER_REFRESH_SECRET` | Signs JWT refresh tokens. Same format as above. |
| `SCOUTER_BOOTSTRAP_KEY` | Used to create the initial admin user on first startup. Also serves as a shared key for inter-service authentication (e.g. OpsML integration). Same format as above. |

To generate a suitable secret:

```bash
openssl rand -base64 32
```

### Object Store Variables

Controls where archived data (expired from PostgreSQL) is written for long-term storage.

| Variable | Description | Default |
|----------|-------------|---------|
| `SCOUTER_STORAGE_URI` | Object store URI. Supports `s3://`, `gs://`, `az://`, or a local path | `./scouter_storage` |
| `AWS_REGION` | AWS region (required for S3) | `us-east-1` |
| `GOOGLE_ACCOUNT_JSON_BASE64` | Base64-encoded GCP service account JSON (optional, for GCS) | — |

**URI examples:**

```bash
# AWS S3
export SCOUTER_STORAGE_URI=s3://my-scouter-bucket

# Google Cloud Storage
export SCOUTER_STORAGE_URI=gs://my-scouter-bucket

# Azure Blob Storage
export SCOUTER_STORAGE_URI=az://my-scouter-container

# Local filesystem (default)
export SCOUTER_STORAGE_URI=./scouter_storage
```

See the [object-store crate](https://docs.rs/object_store/latest/object_store/index.html) docs for provider-specific credential configuration:

- [AWS S3](https://docs.rs/object_store/latest/object_store/aws/struct.AmazonS3Builder.html)
- [Google Cloud Storage](https://docs.rs/object_store/latest/object_store/gcp/struct.GoogleCloudStorageBuilder.html)
- [Azure Blob Storage](https://docs.rs/object_store/latest/object_store/azure/struct.MicrosoftAzureBuilder.html)

### Client Variables

These are used by the Python client (`ScouterClient`, `ScouterQueue`) to connect to the server.

| Variable | Description | Default |
|----------|-------------|---------|
| `SCOUTER_SERVER_URI` | HTTP server address | `http://localhost:8000` |
| `SCOUTER_GRPC_URI` | gRPC server address | `http://localhost:50051` |
| `SCOUTER_USERNAME` | Username for authentication | `guest` |
| `SCOUTER_PASSWORD` | Password for authentication | `guest` |
| `SCOUTER_AUTH_TOKEN` | Pre-issued auth token (alternative to username/password) | — |
