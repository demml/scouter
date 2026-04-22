from pathlib import Path

from scouter.evaluate import EvalOrchestrator, ScenarioEvalResults

from ...non_interactive.google.agent import run_agent as run_google_agent
from ...non_interactive.openai.agent import run_agent as run_openai_agent
from ...non_interactive.shared import get_shared_config, teardown_shared_config

_BASELINE_PATH = Path(__file__).with_name("google_results.json")
_COMPARISON_PATH = Path(__file__).with_name("openai_results.json")


def _run_google() -> ScenarioEvalResults:
    config = get_shared_config()
    results = EvalOrchestrator(
        queue=config.queue,
        scenarios=config.scenarios,
        agent_fn=run_google_agent,
    ).run()
    results.save(str(_BASELINE_PATH))
    return results


def _run_openai() -> ScenarioEvalResults:
    config = get_shared_config()
    results = EvalOrchestrator(
        queue=config.queue,
        scenarios=config.scenarios,
        agent_fn=run_openai_agent,
    ).run()
    results.save(str(_COMPARISON_PATH))
    return results


def main() -> None:
    try:
        _run_google()
        _run_openai()

        baseline = ScenarioEvalResults.load(str(_BASELINE_PATH))
        comparison = ScenarioEvalResults.load(str(_COMPARISON_PATH))
        delta = comparison.compare_to(baseline)
        delta.as_table()
    finally:
        teardown_shared_config()
        if _BASELINE_PATH.exists():
            _BASELINE_PATH.unlink()
        if _COMPARISON_PATH.exists():
            _COMPARISON_PATH.unlink()


if __name__ == "__main__":
    main()
