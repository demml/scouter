.PHONY: build
format:
	cargo fmt --all

.PHONY: lints
lints:
	cargo clippy --workspace --all-targets -- -D warnings


.PHONY: build.postgres
build.sql:
	docker-compose down
	docker-compose up -d --build postgres


.PHONY: test.sql.postgres
test.sql:
	cargo test -p scouter-sql -- --nocapture --test-threads=1