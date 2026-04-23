# Interactive Evaluation

This directory demonstrates reactive evaluation with `EvalOrchestrator`:

- the agent receives one message at a time
- a simulated user decides the next turn based on prior exchanges
- evaluation runs until `termination_signal` is produced or `max_turns` is reached

`agent.py` stays API-ready while `evaluate.py` focuses on orchestration and simulated user logic.

## Run

```bash
cd py-scouter
uv run python -m examples.evaluate.interactive.google.evaluate
uv run python -m examples.evaluate.interactive.openai.evaluate
uv run python -m examples.evaluate.interactive.crewai.evaluate
```

