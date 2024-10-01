
.PHONY: format
format:
	cargo fmt

.PHONY: lint
lints:
	cargo clippy --workspace --all-targets --features "dispatch" -- -D warnings

.PHONY: test
test:
	cargo test --features "dispatch" -- --nocapture  --test-threads=1 

.PHONE: bench
bench:
	cargo bench