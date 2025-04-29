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

<span class="text-primary">**Database Variables**</span>

| Variable | Description |
|----------|-------------|
| DATABASE_URI | The URI of the Postgres database to use for storing and retrieving data. The default is `postgresql://postgres:postgres@localhost:5432/postgres` |
| MAX_POOL_SIZE | The maximum pool size for sqlx to set for the database. Default is 30 |
| DATA_RETENTION_PERIOD | The number of days to keep data in the database. The default is 30 days. After this time, data will be pushed to long-term storage via `DataFusion` |


<span class="text-primary">**Polling Variables (For background drift detection)**</span>

| Variable | Description |
|----------|-------------|
| POLLING_WORKER_COUNT | The number of workers to use to run the background drift and alerting jobs. Default is 4 |

<span class="text-primary">**Kafka Variables (if enabled, for record consumption)**</span>

| Variable | Description |
|----------|-------------|
| KAFKA_BROKERS | A comma-separated list of Kafka brokers to use for the event system. The default is `localhost:9092` |
| KAFKA_WORKER_COUNT | The number of workers to use for consuming events from Kafka. The default is 3 |
| KAFKA_TOPIC | The topic to use for sending events to Kafka. The default is `scouter_monitoring` |
| KAFKA_GROUP | The group ID to use for consuming events from Kafka. The default is `scouter` |
| KAFKA_OFFSET_RESET | The offset reset policy to use for consuming events from Kafka. The default is `earliest` |
| KAFKA_USERNAME | The username to use for authenticating with Kafka |
| KAFKA_PASSWORD | The password to use for authenticating with Kafka |
| KAFKA_SECURITY_PROTOCOL | The security protocol to use for connecting to Kafka. The default is `SASL_SSL` |
| KAFKA_SASL_MECHANISM | The SASL mechanism to use for authenticating with Kafka. The default is `PLAIN` |
| KAFKA_CERT_LOCATION | The location of the CA certificate to use for authenticating with Kafka. The default is None |


<span class="text-primary">**RabbitMQ Variables (if enabled, for record consumptio)**</span>

| Variable | Description |
|----------|-------------|
| RABBITMQ_ADDR| The address of the RabbitMQ server to use for the event system. The default is `amqp://guest:guest@localhost:5672/%2f` |
| RABBITMQ_PREFETCH_COUNT | The number of messages to prefetch from RabbitMQ. The default is 10 |
| RABBITMQ_CONSUMER_COUNT | The number of consumers to use for consuming events from RabbitMQ. The default is 3 |
| RABBITMQ_QUEUE | The queue to use for sending events to RabbitMQ. The default is `scouter_monitoring` |
| RABBITMQ_CONSUMER_TAG | The consumer tag to use for consuming events from RabbitMQ. The default is `scouter` |

<span class="text-primary">**ObjectStore Variables (For long-term storage)**</span>

| Variable | Description |
|----------|-------------|
| SCOUTER_STORAGE_URI | The URI of the object store to use for long-term storage. The default is `./scouter_storage`. Currently, gcs, s3, azure and local storage are supported. If using cloud storage, provide the appropriate prefix and bucket (e.g. gs://scouter, s3://scouter, az://scouter) |
| AWS_REGION | The AWS region to use for S3 storage. The default is `us-east-1` |
| GOOGLE_ACCOUNT_JSON_BASE64 | The base64 encoded JSON key file to use for GCS storage. This is an optional environment variable that can be used in addition to how the `object-store` GoogleCloudStorageBuilder retrieves credentials |

Scouter leverage the [object-store](https://docs.rs/object_store/latest/object_store/index.html) crate for writing to object stores. The following object stores are supported. Please refer to the object-store crate for more information on how to configure each object store.

Providers:

- `google_cloud_storage` - [link](https://docs.rs/object_store/latest/object_store/gcp/struct.GoogleCloudStorageBuilder.html)
- `aws_s3` - [link](https://docs.rs/object_store/latest/object_store/aws/struct.AmazonS3Builder.html)
- `azure` - [link](https://docs.rs/object_store/latest/object_store/azure/struct.MicrosoftAzureBuilder.html)

<span class="text-primary">**Authentication**</span>

| Variable | Description |
|----------|-------------|
| SCOUTER_ENCRYPT_SECRET | The master secret key to use for secure access to the Scouter server. If not set, scouter will use a default **deterministic** key. This is not recommended for production use cases. Souter requires a pbdkdf2::HmacSha256 key with a length of 32 bytes  |
| SCOUTER_REFRESH_SECRET | The refresh key to use for secure access to the Scouter server. If not set, scouter will use a default **deterministic** key. This is not recommended for production use cases. Souter requires a pbdkdf2::HmacSha256 key with a length of 32 bytes  |