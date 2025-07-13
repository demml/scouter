The Scouter server is a rust-based server that is designed to be run independent of the python client. The server is responsible for handling the event system (via Kafka, RabbitMQ or the default queue system), database CRUD operations, and alerting.

Features:

- <span class="text-secondary">**Event System**</span> - Scouter supports various third-party event systems such as Kafka and RabbitMQ. The event system is used to send events to the Scouter server for processing
- <span class="text-secondary">**Database Storage**</span> - The Scouter server leverages **Postgres** (sqlx) for short-term data storage and **DataFusion** for long-term data storage
- <span class="text-secondary">**Alerting**</span> - Integrates with OpsGenie and Slack for alerting
- <span class="text-secondary">**Data Retention and Partitioning**</span> - Built-in data retention and partitioning strategies to keep your database clean and performant
- <span class="text-secondary">**Authentication**</span> - Built-in authentication system for users

## Getting Started

There are a few different ways to get up an running with the Scouter server.

### Prerequisites

- Scouter relies on a **Postgres** database for storing and retrieving data. You will need to have a Postgres database up and running before you can use Scouter. Scouter currently supports Postgres 16.3 and above.

Once you have a Postgres database up and running, set the following environment variable before starting the Scouter server:

```bash
export DATABASE_URI={your_postgres_uri}
```

The default database URI is `postgresql://postgres:postgres@localhost:5432/postgres`. You can change this to point to your own Postgres database.

### Docker

It is recommended to use one of our pre-built Docker images to get up and running quickly. New docker images are built and tagged on every version release and currently build for the following platforms:

<span class="text-primary">**Amd64** (x86_64-unknown-linux-gnu)</span>

| Image | Tag Suffix | Features |
|-------|------------|---------|
| ubuntu | ubuntu | (kafka, RabbitMQ) |
| alpine | alpine | (kafka, RabbitMQ) |
| scratch | scratch | (kafka, RabbitMQ) |
| debian | debian | (kafka, RabbitMQ) |
| distroless | distroless | (kafka, RabbitMQ) |

<span class="text-primary">**Arm64** (aarch64-unknown-linux-gnu)</span>

| Image | Tag Suffix | Features |
|-------|------------|---------|
| ubuntu | ubuntu | (kafka, RabbitMQ) |
| alpine | alpine | (kafka, RabbitMQ) |
| scratch | scratch | (kafka, RabbitMQ) |
| debian | debian | (kafka, RabbitMQ) |
| distroless | distroless | (kafka, RabbitMQ) |


#### Pull your image

```bash
docker pull demml/scouter:ubuntu-amd64-kafka-latest
```

#### Run your image

```bash
docker run -d --name scouter -p 8080:8080 demml/scouter:ubuntu-amd64-kafka-latest
```


### Execute prebuilt binary

Binaries for various architectures are published on every release. You can find them on the github release page, download and execute the binary.

Binaries can be found [here](https://github.com/demml/scouter/releases)


### Build from source

To build the Scouter server from source, you will need to have the following dependencies installed:

- Rust (with Cargo) [link](https://www.rust-lang.org/)

Then you can build the server using the following command:

```bash
cargo build scouter-server --release
```

### Feature Flags

Scouter server is built with a few feature flags that can be enabled or disabled at build time. The following feature flags are available:

- `kafka` - Installs the necessary dependencies to use Kafka as the event system (`rdkafka` crate)

```bash
cargo build scouter-server --release --features "kafka"
```

- `rabbitmq` - Installs the necessary dependencies to use RabbitMQ as the event system (`lapin` crate)

```bash
cargo build scouter-server --release --features "rabbitmq"
```

## Environment Variables

You can set a variety of environment variables to configure the Scouter server to your needs.

### **Database Variables**

| Variable | Description | Feature | Default |
|----------|-------------|---------|---------|
| `DATABASE_URI` | Database connection URL | `All` | `postgresql://postgres:postgres@localhost:5432/postgres` |
| `MAX_POOL_SIZE` | Maximum number of database connections in the pool | `All` | `30` |
| `DATA_RETENTION_PERIOD` | Number of days to retain data in the database. After this time, data will be pushed to long-term storage via `DataFusion` | `All` | `30` |


### **Polling Variables (For background drift detection)**

| Variable | Description | Feature | Default |
|----------|-------------|---------|---------|
| `POLLING_WORKER_COUNT` | Number of polling workers for processing scheduled tasks and alerting | `All` | `4` |

### **Kafka Variables (if enabled, for record consumption)**

| Variable | Description | Feature | Default |
|----------|-------------|---------|---------|
| `KAFKA_BROKERS` | Comma-separated list of Kafka broker addresses | `Kafka` | `localhost:9092` |
| `KAFKA_TOPIC` | Kafka topic for sending data | `Kafka` | `scouter_monitoring` |
| `KAFKA_GROUP` | Kafka consumer group ID | `Kafka` | `scouter` |
| `KAFKA_OFFSET_RESET` | Kafka offset reset policy | `Kafka` | `earliest` |
| `KAFKA_USERNAME` | Kafka username for authentication | `Kafka` | `None` |
| `KAFKA_PASSWORD` | Kafka password for authentication | `Kafka` | `None` |
| `KAFKA_SECURITY_PROTOCOL` | Kafka security protocol (e.g., `PLAINTEXT`, `SSL`, `SASL_PLAINTEXT`, `SASL_SSL`) | `Kafka` | `SASL_SSL` |
| `KAFKA_SASL_MECHANISM` | Kafka SASL mechanism | `Kafka` | `PLAIN` |
| `KAFKA_CERT_LOCATION` | Path to the Kafka CA certificate file | `Kafka` | `None` |


### **RabbitMQ Variables (if enabled, for record consumption)**

| Variable | Description | Feature | Default |
|----------|-------------|---------|---------|
| `RABBITMQ_CONSUMER_COUNT` | Number of RabbitMQ consumers | `RabbitMQ` | `3` |
| `RABBITMQ_PREFETCH_COUNT` | Number of messages to prefetch per consumer | `RabbitMQ` | `10` |
| `RABBITMQ_ADDR` | RabbitMQ server address | `RabbitMQ` | `amqp://guest:guest@127.0.0.1:5672/%2f` |
| `RABBITMQ_QUEUE` | RabbitMQ queue name | `RabbitMQ` | `scouter_monitoring` |
| `RABBITMQ_CONSUMER_TAG` | RabbitMQ consumer tag | `RabbitMQ` | `scouter` |


### **Authentication**

Scouter has built-in authentication support for JWT tokens. You can set the following environment variables to enable authentication:

- `SCOUTER_ENCRYPT_SECRET`: Secret key used to sign JWT tokens. If not set, scouter will use a default **deterministic** key. This is not recommended for production use cases. Scouter requires a pbdkdf2::HmacSha256 key with a length of 32 bytes.
- `SCOUTER_REFRESH_SECRET`: Secret key used to sign refresh tokens. If not set, scouter will use a default **deterministic** key. This is not recommended for production use cases. Scouter requires a pbdkdf2::HmacSha256 key with a length of 32 bytes.
- `SCOUTER_BOOTSTRAP_KEY`: Secret key used to bootstrap the server. This is used to create the initial admin user and should be set to a strong random value. If not set, scouter will use a default. This is also a pbdkdf2::HmacSha256 key with a length of 32 bytes. This can be used as a shared key for integration work as well. For example, when setting up an [opsml](https://docs.demml.io/opsml/docs/setup/overview/) server, you can user this key to sync user accounts between the two servers.

### **ObjectStore Variables (For long-term storage)**

| Variable | Description | Feature | Default |
|----------|-------------|---------|---------|
| `SCOUTER_STORAGE_URI` | The URI of the object store to use for long-term storage. The default is `./scouter_storage`. Currently, gcs, s3, azure and local storage are supported. If using cloud storage, provide the appropriate prefix and bucket (e.g. gs://scouter, s3://scouter, az://scouter) | `ObjectStore` | `./scouter_storage` |
| `AWS_REGION` | The AWS region to use for S3 storage. The default is `us-east-1` | `ObjectStore` | `us-east-1` |
| `GOOGLE_ACCOUNT_JSON_BASE64` | The base64 encoded JSON key file to use for GCS storage. This is an optional environment variable that can be used in addition to how the `object-store` GoogleCloudStorageBuilder retrieves credentials | `ObjectStore` | `None` |


## Object Store Providers
Scouter leverage's the [object-store](https://docs.rs/object_store/latest/object_store/index.html) crate for writing to object stores. The following object stores are supported. Please refer to the object-store crate for more information on how to configure each object store.

Providers:

- `google_cloud_storage` - [link](https://docs.rs/object_store/latest/object_store/gcp/struct.GoogleCloudStorageBuilder.html)
- `aws_s3` - [link](https://docs.rs/object_store/latest/object_store/aws/struct.AmazonS3Builder.html)
- `azure` - [link](https://docs.rs/object_store/latest/object_store/azure/struct.MicrosoftAzureBuilder.html)

