
.PHONY: format
format:
	cargo fmt

.PHONY: lint
lints:
	cargo clippy --workspace --all-targets -- -D warnings

.PHONY: test
test:
	cargo test -- --nocapture  --test-threads=1

.PHONE: bench
bench:
	cargo bench