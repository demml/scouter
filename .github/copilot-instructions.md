# Copilot Code Review Instructions

## Project: Scouter

Monitoring toolkit for ML/AI workflows. Architecture: Rust server (Axum HTTP + Tonic gRPC) with a Python client via PyO3. PostgreSQL for recent data, DataFusion for archival. Async throughout (Tokio).

## Key Crates

| Crate | Purpose |
|-------|---------|
| `scouter-server` | HTTP/gRPC entrypoint, Axum routes |
| `scouter-sql` | sqlx, PostgreSQL, background workers |
| `scouter-drift` | PSI/SPC/custom drift algorithms |
| `scouter-tracing` | OpenTelemetry span ingestion |
| `scouter-evaluate` | Agent eval tasks (LLM judge, assertions) |
| `scouter-auth` | JWT authentication |
| `scouter-types` | Shared contracts across crates |

## Absolute Rules

- Rust: always `--all-features`; clippy `-D warnings` (all warnings are errors)
- Python tests: top-level `def test_*` only — **never** `class TestFoo:`
- No over-engineering. No speculative abstractions.
- Single responsibility: functions that do two things should be split.
- Do not add comments, docstrings, or type annotations to untouched code.

## Security

- Every new Axum route and Tonic handler must have auth middleware applied.
- No `unwrap()`/`expect()` on values derived from external input or gRPC fields.
- No raw string interpolation in sqlx queries — parameterized only.
- No secrets, tokens, or PII logged at any level (including DEBUG).
- `unsafe` blocks must be minimal and have an explanatory comment.
- Integer arithmetic on externally-supplied values must use `checked_*` or `saturating_*`.
- Unauthenticated endpoints with no rate limiting are a MAJOR finding.

## Correctness & Performance

- `.await` inside a `Mutex` guard → deadlock risk → **CRITICAL**
- Blocking I/O (`std::fs`, `std::thread::sleep`) on async threads → **CRITICAL**
- `unwrap()`/`expect()` on realistic `None`/`Err` paths → **MAJOR**
- N+1 queries: loops with per-item DB calls → **MAJOR**
- `clone()` in hot paths where a reference suffices → **MINOR**
- Regex or expensive objects compiled inside loops → **MINOR**

## Style & Conventions

- Rust errors: `thiserror` for error types; propagate with `?`
- PyO3: `pyo3(get, set)`, `pyo3(signature)`, and module registration in `lib.rs` must match existing patterns in `py-scouter/src/`.
- Structs implementing `Default`: use `..Default::default()` in tests, not field-by-field construction.
- New public API shape must be consistent with adjacent existing APIs in the same module.
- Do not introduce new architectural paradigms without strong justification.

## Tests

- New public Rust functions need a `#[cfg(test)]` module with at least one test.
- New Python functions need `def test_*` coverage — no class-based wrappers.
- SQL tests must use `--test-threads=1` for isolation.
- New HTTP/gRPC endpoints need integration test coverage.
- Tests must assert meaningful behavior, not just that code runs without panicking.

## PyO3 Boundary

- All types crossing the Rust↔Python boundary must implement `IntoPy`/`FromPyObject` correctly.
- GIL acquisition patterns must match existing usage in `py-scouter/src/`.
- After any Rust change, `make setup.project` (in `py-scouter/`) rebuilds the extension.

## Severity Guide

| Level | Meaning |
|-------|---------|
| **CRITICAL** | Panic, data loss, auth bypass, deadlock, secret leak |
| **MAJOR** | Incorrect behavior a real user triggers; blocking I/O on async; missing auth |
| **MINOR** | Style inconsistency, non-critical performance, theoretical edge case |

Only flag issues with a concrete failure path under realistic production conditions.
An empty report is correct when the code is solid.
