.PHONY: build
format:
	cargo fmt --all

.PHONY: lints
lints:
	cargo clippy --workspace --all-targets -- -D warnings


.PHONY: build.sql
build.sql:
	docker-compose down
	docker-compose up -d --build postgres

.PHONY: test.sql
test.sql:
	cargo test -p scouter-sql test_postgres -- --nocapture --test-threads=1

.PHONY: test.types
test.types:
	cargo test -p scouter-types -- --nocapture --test-threads=1

.PHONY: test.dispatch
test.dispatch:
	cargo test -p scouter-dispatch -- --nocapture --test-threads=1

.PHONY: test.drift
test.drift:
	cargo test -p scouter-drift --all-features -- --nocapture --test-threads=1

.PHONY: test.drift.executor
test.drift.executor:
	cargo test -p scouter-drift test_drift_executor --all-features -- --nocapture --test-threads=1


.PHONY: test.profile
test.profile:
	cargo test -p scouter-profile -- --nocapture --test-threads=1

.PHONY: test.server
test.server:
	cargo test -p scouter-server -- --nocapture --test-threads=1