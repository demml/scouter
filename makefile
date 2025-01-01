.PHONY: build
format:
	cargo fmt --all

.PHONY: lints
lints:
	cargo clippy --workspace --all-targets -- -D warnings