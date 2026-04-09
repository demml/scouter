from scouter.evaluate import (
    AssertionTask,
    ComparisonOperator,
    EvalScenario,
    EvalScenarios,
)

# simulated_user_persona marks these as reactive — the eval loop drives
# the conversation until the customer agent outputs the termination_signal.
scenarios = EvalScenarios(
    scenarios=[
        EvalScenario(
            id="quick_dinner",
            initial_query="I need dinner ideas for tonight.",
            expected_outcome="A complete recipe with ingredients and steps",
            simulated_user_persona="busy home cook with limited time",
            termination_signal="SATISFIED",
            max_turns=6,
            tasks=[
                AssertionTask(
                    id="final_response_is_string",
                    context_path="response",
                    operator=ComparisonOperator.IsString,
                    expected_value=True,
                ),
            ],
        ),
        EvalScenario(
            id="vegetarian_pasta",
            initial_query="What can I make with vegetables and pasta?",
            expected_outcome="A vegetarian pasta recipe with full instructions",
            simulated_user_persona="beginner cook interested in vegetarian cooking",
            termination_signal="SATISFIED",
            max_turns=6,
            tasks=[
                AssertionTask(
                    id="final_response_is_string",
                    context_path="response",
                    operator=ComparisonOperator.IsString,
                    expected_value=True,
                ),
            ],
        ),
    ]
)
