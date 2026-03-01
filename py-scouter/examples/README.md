# Scouter Python Examples

Runnable examples covering the three main Scouter workflows: data profiling, online drift monitoring, and offline GenAI evaluation.

## Prerequisites

- A running Scouter server (see [server setup](../docs/docs/server/index.md))
- Python 3.10+, managed via `uv`

```bash
# From py-scouter/
make setup.project   # builds the Rust extension and syncs Python deps
```

Point the client at your server:

```bash
export SCOUTER_SERVER_URI=http://localhost:8000
export SCOUTER_USERNAME=admin
export SCOUTER_PASSWORD=admin
```

## Examples

| Directory | What it covers |
|-----------|---------------|
| [`profile/`](profile/) | Computing a `DataProfile` from a Pandas DataFrame |
| [`monitor/`](monitor/) | Online drift monitoring via a live FastAPI service |
| [`evaluate/`](evaluate/) | Offline batch GenAI evaluation with `AssertionTask` and `LLMJudgeTask` |

## Running an example

```bash
cd py-scouter
uv run python examples/profile/pandas_dataframe.py
uv run python examples/evaluate/customer_support.py
```

For the FastAPI monitor examples, a running server is required and the profile must be registered first — see the [`monitor/` README](monitor/README.md).
