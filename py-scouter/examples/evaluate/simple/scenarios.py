from scouter.evaluate import (
    AssertionTask,
    ComparisonOperator,
    EvalScenario,
    EvalScenarios,
)

# ---------------------------------------------------------------------------
#    Define the evaluation scenarios.
#    tasks in each scenario run assertions against the agent's response string.
# ---------------------------------------------------------------------------
scenarios = EvalScenarios(
    scenarios=[
        EvalScenario(
            id="capital_france",
            initial_query="What is the capital of France?",
            expected_outcome="Paris",
            tasks=[
                AssertionTask(
                    id="mentions_paris",
                    context_path="response",
                    operator=ComparisonOperator.Contains,
                    expected_value="Paris",
                ),
            ],
        ),
        EvalScenario(
            id="water_formula",
            initial_query="What is the chemical formula for water?",
            expected_outcome="H2O",
            tasks=[
                AssertionTask(
                    id="mentions_h2o",
                    context_path="response",
                    operator=ComparisonOperator.Contains,
                    expected_value="H2O",
                ),
            ],
        ),
        EvalScenario(
            id="speed_of_light",
            initial_query="What is the speed of light in a vacuum?",
            expected_outcome="approximately 300,000 km/s",
            tasks=[
                AssertionTask(
                    id="mentions_speed",
                    context_path="response",
                    operator=ComparisonOperator.Contains,
                    expected_value="km/s",
                ),
            ],
        ),
    ]
)
