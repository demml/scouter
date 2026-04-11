from pathlib import Path

from scouter.evaluate import EvalScenarios

scenarios = EvalScenarios.from_path(Path(__file__).parent / "scenarios.jsonl")
