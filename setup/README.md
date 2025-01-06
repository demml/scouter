# Setting up postgres

By default, Scouter-Server uses postgres and is configured to run with both `pg_partman` and `pg_cron` extensions in order to manage partitions and schedule tasks. For an example of setting this up with a local postgres instance, see the [docker-compose.yml](../docker-compose.yml) and [Dockerfile](../Dockerfile) files.

## User Roles and DB Config

Upon application startup, Scouter-Server will attempt to run an initial migration to setup database tables, partitioning and cron configurations. Prior to application startup, you will need to create the `scouter` schema, as well as the `pg_partman` and `pg_cron` extensions and create a user and database for Scouter-Server to use (unless you are using a superuser). If using a non-superuser when running the application, the user will need to have a variety of permissions on the database and the `scouter` and `cron` schemas.

## This must be run by a superuser when the database is created
```sql
CREATE SCHEMA if not exists scouter;
CREATE EXTENSION if not exists pg_partman SCHEMA scouter;
CREATE EXTENSION if not exists pg_cron;
```


## Example non-superuser permissions

- This was tested on cloudsql with postgres 15

```sql
-- create user
CREATE USER my_user WITH PASSWORD 'my_pass';

-- scouter schema
GRANT ALL ON SCHEMA scouter TO my_user;
GRANT ALL ON ALL TABLES IN SCHEMA scouter TO my_user;
GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA scouter to my_user;
GRANT EXECUTE ON ALL PROCEDURES IN SCHEMA scouter TO my_user;
GRANT ALL PRIVILEGES ON TABLE scouter.part_config_sub TO my_user;
GRANT ALL PRIVILEGES ON TABLE scouter.part_config TO my_user;

-- pg cron
GRANT USAGE ON SCHEMA cron TO my_user;
GRANT ALL ON ALL TABLES IN SCHEMA cron TO my_user;
```

## Env Variables

### Database

`DATABASE_URL` - The connection string for the database. Default is `postgresql://postgres:admin@localhost:5432/scouter?`

`MAX_CONNECTIONS` - The maximum number of connections to the database. Default is 10. These connections are pooled and reused.

`SCOUTER_SERVER_PORT` - The port to run the server on. Default is 8000.

### Polling and Scheduling

`SCOUTER_SCHEDULE_WORKER_COUNT` - Number of workers to run scheduled tasks. Default is 4.

### Kafka (Optional Feature)

`KAFKA_BROKERS` - The kafka brokers to connect to. Default is `localhost:9092`.

`KAFKA_WORKER_COUNT` - Number of workers to run kafka tasks. Default is 3.

`KAFKA_TOPIC` - The kafka topic to listen to. Default is `scouter_monitoring`.

`KAFKA_GROUP` - The kafka group to listen to. Default is `scouter`.

`KAFKA_USERNAME` - The username for the kafka connection (Optional).

`KAFKA_PASSWORD` - The password for the kafka connection (Optional).

`KAFKA_SECURITY_PROTOCOL` - The security protocol for the kafka connection. Default is `SASL_SSL`.

`KAFKA_SASL_MECHANISM` - The SASL mechanism for the kafka connection. Default is `PLAIN`.

### RabbitMQ (Optional Feature)

`RABBITMQ_ADDR` - The rabbitmq address to connect to. Default is `amqp://guest:guest@127.0.0.1:5672/%2f`.

`RABBITMQ_CONSUMERS_COUNT` - Number of consumers to run rabbitmq tasks. Default is 3.

`RABBITMQ_PREFETCH_COUNT` - The number of messages to prefetch. Default is 1.