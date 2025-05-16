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

For more information on the server setup, Please see the documentation.