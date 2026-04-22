# Non-Interactive Offline Evaluation

This directory shows the pre-deploy evaluation loop:

1. Build an API-friendly agent implementation in `agent.py`.
2. Emit `EvalRecord` data from the same execution path used by the API.
3. Run fixed scenario evaluation with `EvalOrchestrator`.

`shared/` contains common prompt/tasks/scenarios so framework examples differ only by adapter code.

## Run

```bash
cd py-scouter
uv run python -m examples.evaluate.non_interactive.google.evaluate
uv run python -m examples.evaluate.non_interactive.openai.evaluate
uv run python -m examples.evaluate.non_interactive.crewai.evaluate
```

