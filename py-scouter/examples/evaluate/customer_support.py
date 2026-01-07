from pydantic import BaseModel
from scouter.evaluate import (
    AssertionTask,
    ComparisonOperator,
    GenAIEvalDataset,
    LLMJudgeTask,
)
from scouter.genai import Prompt
from scouter.queue import GenAIEvalRecord


class SupportResponse(BaseModel):
    response: str
    addresses_issue: bool
    provides_solution: bool
    confidence_score: float


class ResponseQuality(BaseModel):
    is_helpful: bool
    reason: str


customer_issue = (
    "I've been trying to reset my password for 30 minutes but I'm not "
    "receiving the reset email. I need access urgently for a meeting in 2 hours!"
)

# Your agent generates a response
support_response = SupportResponse(
    response="I apologize for the frustration. Let me help you immediately...",
    addresses_issue=True,
    provides_solution=True,
    confidence_score=0.85,
)


quality_prompt = Prompt(
    messages=(
        "Evaluate this customer support response:\n\n"
        "Customer Issue: ${customer_issue}\n"
        "Support Response: ${response}\n\n"
        "Is the response helpful and professional? "
        "Return a JSON with 'is_helpful' (boolean) and 'reason' (string)."
    ),
    model="gemini-2.5-flash-lite",
    provider="gemini",
    output_type=ResponseQuality,  # Your evaluation model
)

record = GenAIEvalRecord(
    context={"customer_issue": customer_issue, "response": support_response},
)


dataset = GenAIEvalDataset(
    records=[record],
    tasks=[
        # LLM Judge: Subjective quality assessment
        LLMJudgeTask(
            id="response_helpful",
            prompt=quality_prompt,
            expected_value=True,
            operator=ComparisonOperator.Equals,
            field_path="is_helpful",
            description="Verify the response is helpful",
        ),
        # Assertions: Objective checks
        AssertionTask(
            id="addresses_issue",
            field_path="response.addresses_issue",
            operator=ComparisonOperator.Equals,
            expected_value=True,
            description="Verify response addresses the issue",
        ),
        AssertionTask(
            id="provides_solution",
            field_path="response.provides_solution",
            operator=ComparisonOperator.Equals,
            expected_value=True,
            description="Verify response provides a solution",
        ),
        AssertionTask(
            id="high_confidence",
            field_path="response.confidence_score",
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=0.7,
            description="Verify confidence is high (>=0.7)",
        ),
    ],
)

print("\n=== Evaluation Plan ===")
dataset.print_execution_plan()

# Run all evaluations
print("\n=== Running Evaluation ===")
results = dataset.evaluate()
results.as_table(show_tasks=True)
