# need to turin of E402 - we need to call set_offline before importing the shared config and service builder
# ruff: noqa: E402
# pylint: disable=wrong-import-position
from scouter import ScouterEnv

ScouterEnv.set_offline()

from scouter.evaluate import EvalOrchestrator

from ..shared import get_shared_config, teardown_shared_config
from .agent import run_agent


def main() -> None:
    config = get_shared_config()
    try:
        results = EvalOrchestrator(
            queue=config.queue,
            scenarios=config.scenarios,
            agent_fn=run_agent,
        ).run()
    finally:
        teardown_shared_config()

    results.as_table(show_workflow=True)


if __name__ == "__main__":
    main()
