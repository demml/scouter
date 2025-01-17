.PHONY: build
format:
	cargo fmt --all

.PHONY: lints
lints:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

# build for kafka
.PHONY: build.kafka
build.kafka:
	docker compose down
	docker compose up -d --build init-kafka --wait

# For tests that need postgres
.PHONY: build.sql
build.sql:
	docker compose down
	docker compose up -d --build postgres --wait
	
.PHONY: test.sql
test.sql:
	cargo test -p scouter-sql test_postgres -- --nocapture --test-threads=1

.PHONY: test.server
test.server:
	cargo test -p scouter-server --all-features -- --nocapture --test-threads=1

.PHONY: test.drift.executor
test.drift.executor:
	cargo test -p scouter-drift test_drift_executor --all-features -- --nocapture --test-threads=1

.PHONY: test.needs_sql
test.needs_sql: build.sql test.sql test.server test.drift.executor
	docker compose down

#### Unit tests
.PHONY: test.types
test.types:
	cargo test -p scouter-types -- --nocapture --test-threads=1

.PHONY: test.dispatch
test.dispatch:
	cargo test -p scouter-dispatch -- --nocapture --test-threads=1

.PHONY: test.drift
test.drift:
	cargo test -p scouter-drift --all-features -- --nocapture --test-threads=1 --skip test_drift_executor

.PHONY: test.profile
test.profile:
	cargo test -p scouter-profile -- --nocapture --test-threads=1

.PHONY: test.unit
test.unit: test.types test.dispatch test.drift test.profile

#### Event tests
.PHONY: build.sql_kafka
build.sql_kafka:
	docker compose down
	docker compose up -d --build postgres-kafka --wait

.PHONY: test.kafka_events
test.kafka_events: build.sql_kafka
	cargo run --example kafka_integration --all-features -- --nocapture


.PHONY: build.sql_rabbitmq
build.sql_rabbitmq:
	docker compose down
	docker compose up -d --build postgres-rabbitmq --wait

.PHONY: test.rabbitmq_events
test.rabbitmq_events: build.sql_rabbitmq
	cargo run --example rabbitmq_integration --all-features -- --nocapture

.PHONY: test.integration
test.events: test.kafka_events test.rabbitmq_events

.PHONY: test
test: test.needs_sql test.unit


.PHONY: shutdown
shutdown:
	docker compose down



###### Server tests

.PHONY: build.all_backends
build.all_backends:
	docker compose down
	docker compose up -d --build server-backends --wait

.PHONE: build.server
build.server: build.all_backends
	cargo build -p scouter-server --all-features
	./target/debug/scouter-server &


.PHONY: build.shutdown_backends
build.shutdown_backends:
	docker compose down


.PHONE: stop.server
stop.server: build.shutdown_backends
	lsof -ti:8000 | xargs kill -9