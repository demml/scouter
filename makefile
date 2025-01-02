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