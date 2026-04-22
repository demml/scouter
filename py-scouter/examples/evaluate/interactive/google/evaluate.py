from scouter.evaluate import EvalOrchestrator

from ..shared import get_shared_config, teardown_shared_config
from .agent import run_agent


def simulated_user_turn(initial_query: str, agent_response: str, history: list[dict[str, str]]) -> str:
    del initial_query

    if len(history) >= 2:
        return "DONE"

    response_text = str(agent_response).lower()
    if "step" not in response_text:
        return "Give me concrete step-by-step actions and end with DONE when complete."
    return "Add one risk to watch for and then return DONE."


def main() -> None:
    config = get_shared_config()
    try:
        results = EvalOrchestrator(
            queue=config.queue,
            scenarios=config.scenarios,
            agent_fn=run_agent,
            simulated_user_fn=simulated_user_turn,
        ).run()
    finally:
        teardown_shared_config()

    print(
        f"\nScenarios : {results.metrics.total_scenarios}  "
        f"Passed    : {results.metrics.passed_scenarios}  "
        f"Pass rate : {results.metrics.overall_pass_rate:.0%}"
    )
    results.as_table(show_workflow=True)


if __name__ == "__main__":
    main()
