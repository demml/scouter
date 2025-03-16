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
	docker compose up --build postgres --wait
	
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
test.needs_sql: test.sql test.server test.drift.executor

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

.PHONY: test.kafka_events
test.kafka_events:
	cargo run --example kafka_integration --all-features -- --nocapture

.PHONY: test.rabbitmq_events
test.rabbitmq_events:
	cargo run --example rabbitmq_integration --all-features -- --nocapture

.PHONY: test.events
test.events: test.kafka_events test.rabbitmq_events

.PHONY: test
test: build.all_backends test.needs_sql test.unit build.shutdown_backends

###### Server tests
.PHONY: build.all_backends
build.all_backends:
	docker compose down
	docker compose up -d --build server-backends --wait

.PHONE: start.server
start.server: build.all_backends
	export KAFKA_BROKERS=localhost:9092 && \
	export RABBITMQ_ADDR=amqp://guest:guest@127.0.0.1:5672/%2f && \
	cargo build -p scouter-server --all-features && \
	./target/debug/scouter-server &

.PHONY: build.shutdown_backends
build.shutdown:
	docker compose down

.PHONE: stop.server
stop.server: build.shutdown_backends
	lsof -ti:8000 | xargs kill -9