# Evaluation Examples

This directory is organized around the workflows teams use when building and shipping agents with Scouter:

1. Instrument an API-style agent with `ScouterInstrumentor` and OpenTelemetry.
2. Run non-interactive offline evaluation against fixed scenarios.
3. Run interactive evaluation with simulated user turns.
4. Compare two agents on the same tasks and scenarios.

## Layout

- `non_interactive/`: fixed-scenario offline evaluation.
- `interactive/`: reactive evaluation with `simulated_user_fn`.
- `comparison/`: regression checks between agent variants.

## Run

```bash
cd py-scouter
uv run python -m examples.evaluate.non_interactive.google.evaluate
uv run python -m examples.evaluate.interactive.google.evaluate
uv run python -m examples.evaluate.comparison.google_vs_openai.evaluate
```

Each framework folder includes:

- `agent.py`: deployable FastAPI agent surface.
- `evaluate.py`: `EvalOrchestrator` entrypoint that imports `run_agent` from `agent.py`.

