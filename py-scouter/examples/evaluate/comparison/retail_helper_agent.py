"""
This example demonstrates offline evaluations and comparison analysis for a retail
customer support agent.

This example generates baseline and improved evaluation runs for a customer support
agent that handles product inquiries, then compares them to detect improvements and
regressions in response quality, helpfulness, and accuracy.
"""

from typing import List

from pydantic import BaseModel
from scouter.evaluate import (
    AssertionTask,
    ComparisonOperator,
    GenAIEvalDataset,
    LLMJudgeTask,
)
from scouter.genai import Prompt
from scouter.logging import LoggingConfig, LogLevel, RustyLogger
from scouter.queue import GenAIEvalRecord

RustyLogger.setup_logging(LoggingConfig(log_level=LogLevel.Info))


# Data Models for Customer Support Interactions and Evaluations
class CustomerQuery(BaseModel):
    question: str
    customer_context: str
    urgency: str


class AgentResponse(BaseModel):
    answer: str
    provides_solution: bool
    acknowledges_concern: bool
    follow_up_offered: bool


class ResponseQuality(BaseModel):
    is_helpful: bool
    is_professional: bool
    reason: str
    quality_score: int


class TechnicalAccuracy(BaseModel):
    is_accurate: bool
    reason: str
    accuracy_score: int


class EmpathyAssessment(BaseModel):
    shows_empathy: bool
    reason: str
    empathy_score: int


def generate_baseline_interactions() -> List[tuple[CustomerQuery, AgentResponse]]:
    """
    Simulates baseline agent responses with some quality issues. These are going to be
    injected as contexts for the baseline evaluation run.

    Simulated flow:
    CustomerQuery --> SupportAgent(what we're simulating) --> AgentResponse
    """
    return [
        (
            CustomerQuery(
                question="My order hasn't arrived and it's been 2 weeks. What should I do?",
                customer_context="Premium customer, order value $500",
                urgency="high",
            ),
            AgentResponse(
                answer="Your order should arrive soon. Please wait a few more days. If it doesn't arrive, contact us again.",
                provides_solution=False,
                acknowledges_concern=False,
                follow_up_offered=False,
            ),
        ),
        (
            CustomerQuery(
                question="Can I return an item if I opened the package?",
                customer_context="New customer, order value $50",
                urgency="low",
            ),
            AgentResponse(
                answer="Yes, you can return items within 30 days. Make sure the product is in original condition.",
                provides_solution=True,
                acknowledges_concern=True,
                follow_up_offered=False,
            ),
        ),
        (
            CustomerQuery(
                question="I was charged twice for my order. How do I get a refund?",
                customer_context="Regular customer, order value $200",
                urgency="high",
            ),
            AgentResponse(
                answer="I see duplicate charges on your account. I'll process a refund for one charge immediately. It should appear in 5-7 business days.",
                provides_solution=True,
                acknowledges_concern=True,
                follow_up_offered=False,
            ),
        ),
        (
            CustomerQuery(
                question="What's your warranty policy on electronics?",
                customer_context="New customer, browsing",
                urgency="low",
            ),
            AgentResponse(
                answer="Electronics have a 1 year warranty. Contact manufacturer for issues.",
                provides_solution=True,
                acknowledges_concern=False,
                follow_up_offered=False,
            ),
        ),
        (
            CustomerQuery(
                question="I received the wrong item. This is urgent as I need it for tomorrow.",
                customer_context="Premium customer, order value $300",
                urgency="high",
            ),
            AgentResponse(
                answer="I apologize for the mix-up. I can ship the correct item overnight at no charge and arrange pickup of the wrong item.",
                provides_solution=True,
                acknowledges_concern=True,
                follow_up_offered=True,
            ),
        ),
    ]


def generate_improved_interactions() -> List[AgentResponse]:
    """
    Simulates improved agent responses with better quality and empathy.

    We'll use these to create a second evaluation run to compare against the baseline. Think of
    this scenario as we're deploying an improved version of the customer support agent and want to
    verify the improvements via offline evaluation.

    We are evaluating the same CustomerQuery inputs as in the baseline, but with improved AgentResponse outputs.
    """
    return [
        AgentResponse(
            answer="I sincerely apologize for the delay in your order. I've checked your tracking and see it's been stuck in transit. "
            "I'm immediately shipping a replacement with overnight delivery at no charge. You should receive it tomorrow. "
            "I'll also issue a $50 credit to your account for the inconvenience. Would you like me to follow up with you once it's delivered?",
            provides_solution=True,
            acknowledges_concern=True,
            follow_up_offered=True,
        ),
        AgentResponse(
            answer="Yes, absolutely! We accept returns within 30 days even if the package is opened. "
            "As long as the product is unused and in resalable condition, you'll receive a full refund. "
            "I can email you a prepaid return label right now. Would that work for you?",
            provides_solution=True,
            acknowledges_concern=True,
            follow_up_offered=True,
        ),
        AgentResponse(
            answer="I'm so sorry about the duplicate charge - that must be frustrating! I've verified the error on your account. "
            "I'm processing an immediate refund for the duplicate charge, and it will appear in your account within 3-5 business days. "
            "I've also added a $20 credit to your account for the inconvenience. I'll send you a confirmation email with the refund details. "
            "Is there anything else I can help you with today?",
            provides_solution=True,
            acknowledges_concern=True,
            follow_up_offered=True,
        ),
        AgentResponse(
            answer="Great question! All our electronics come with a 1-year manufacturer warranty covering defects and malfunctions. "
            "We also offer optional extended warranties for 2 or 3 years at purchase. If you have any issues, you can contact either "
            "us or the manufacturer directly - we're happy to help coordinate the process. Are you looking at a specific product?",
            provides_solution=True,
            acknowledges_concern=True,
            follow_up_offered=True,
        ),
        AgentResponse(
            answer="I completely understand the urgency and I'm really sorry for this error. I'm immediately shipping the correct item "
            "with guaranteed overnight delivery - you'll have it by 10am tomorrow. I've also arranged a pickup for the incorrect item "
            "at your convenience, no need to go to a store. Additionally, I'm applying a 20% discount to your order for the trouble. "
            "I'll personally monitor the shipment and text you the tracking number within the hour. Does this solution work for you?",
            provides_solution=True,
            acknowledges_concern=True,
            follow_up_offered=True,
        ),
    ]


def create_quality_evaluation_prompt() -> Prompt:
    """Prompt to use with LLMJudgeTask for evaluating response quality."""
    return Prompt(
        messages=(
            "You are an expert customer service quality evaluator. Assess the helpfulness and professionalism "
            "of this customer support response.\n\n"
            "Customer Question:\n"
            "${customer_query}\n\n"
            "Agent Response:\n"
            "${agent_response}\n\n"
            "Evaluate:\n"
            "- is_helpful: Does the response actually help solve the customer's problem?\n"
            "- is_professional: Is the tone appropriate and professional?\n"
            "- reason: Explain your assessment\n"
            "- quality_score: Rate overall quality from 1-10\n\n"
            "Evaluation:"
        ),
        model="gemini-2.5-flash-lite",
        provider="gemini",
        output_type=ResponseQuality,
    )


def create_technical_accuracy_prompt() -> Prompt:
    """Prompt to use with LLMJudgeTask for evaluating technical accuracy."""
    return Prompt(
        messages=(
            "You are a customer service technical accuracy expert. Evaluate if the agent's response "
            "contains accurate information and appropriate solutions.\n\n"
            "Customer Question:\n"
            "${customer_query}\n\n"
            "Agent Response:\n"
            "${agent_response}\n\n"
            "Evaluate:\n"
            "- is_accurate: Is the information technically correct?\n"
            "- reason: Explain your assessment\n"
            "- accuracy_score: Rate accuracy from 1-10\n\n"
            "Evaluation:"
        ),
        model="gemini-2.5-flash-lite",
        provider="gemini",
        output_type=TechnicalAccuracy,
    )


def create_empathy_assessment_prompt() -> Prompt:
    """Prompt to use with LLMJudgeTask for evaluating empathy in responses."""
    return Prompt(
        messages=(
            "You are a customer experience specialist focusing on empathy and emotional intelligence. "
            "Evaluate if the agent's response shows appropriate empathy for the customer's situation.\n\n"
            "Customer Context: ${customer_context}\n"
            "Urgency Level: ${urgency}\n\n"
            "Customer Question:\n"
            "${customer_query}\n\n"
            "Agent Response:\n"
            "${agent_response}\n\n"
            "Evaluate:\n"
            "- shows_empathy: Does the response acknowledge the customer's feelings and concerns?\n"
            "- reason: Explain your assessment\n"
            "- empathy_score: Rate empathy level from 1-10\n\n"
            "Evaluation:"
        ),
        model="gemini-2.5-flash-lite",
        provider="gemini",
        output_type=EmpathyAssessment,
    )


def create_evaluation_tasks() -> List[LLMJudgeTask | AssertionTask]:
    """Standard set of evaluation tasks for customer support agent evaluations."""
    return [
        LLMJudgeTask(
            id="quality_evaluation",
            prompt=create_quality_evaluation_prompt(),
            expected_value=True,
            operator=ComparisonOperator.Equals,
            field_path="is_helpful",
            description="Verify response is helpful to customer",
        ),
        AssertionTask(
            id="quality_score_threshold",
            field_path="quality_evaluation.quality_score",
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=7,
            description="Quality score must be at least 7/10",
            depends_on=["quality_evaluation"],
        ),
        LLMJudgeTask(
            id="technical_accuracy",
            prompt=create_technical_accuracy_prompt(),
            expected_value=True,
            operator=ComparisonOperator.Equals,
            field_path="is_accurate",
            description="Verify technical accuracy of response",
        ),
        AssertionTask(
            id="accuracy_score_threshold",
            field_path="technical_accuracy.accuracy_score",
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=8,
            description="Accuracy score must be at least 8/10",
            depends_on=["technical_accuracy"],
        ),
        LLMJudgeTask(
            id="empathy_assessment",
            prompt=create_empathy_assessment_prompt(),
            expected_value=True,
            operator=ComparisonOperator.Equals,
            field_path="shows_empathy",
            description="Verify response shows appropriate empathy",
        ),
        AssertionTask(
            id="empathy_score_threshold",
            field_path="empathy_assessment.empathy_score",
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=6,
            description="Empathy score must be at least 6/10",
            depends_on=["empathy_assessment"],
        ),
        AssertionTask(
            id="provides_solution",
            field_path="agent_response.provides_solution",
            operator=ComparisonOperator.Equals,
            expected_value=True,
            description="Response must provide a solution",
        ),
        AssertionTask(
            id="acknowledges_concern",
            field_path="agent_response.acknowledges_concern",
            operator=ComparisonOperator.Equals,
            expected_value=True,
            description="Response must acknowledge customer concern",
        ),
    ]


def create_baseline_dataset() -> GenAIEvalDataset:
    """Creates the baseline evaluation dataset for the customer support agent."""
    baseline_data = generate_baseline_interactions()

    records = []
    for idx, (query, response) in enumerate(baseline_data):
        record = GenAIEvalRecord(
            context={
                "customer_query": query.question,
                "customer_context": query.customer_context,
                "urgency": query.urgency,
                "agent_response": response,
            },
            id=f"support_interaction_{idx}",
        )
        records.append(record)

    tasks = create_evaluation_tasks()

    return GenAIEvalDataset(records=records, tasks=tasks)


def main():
    print("\n" + "=" * 80)
    print("BASELINE CUSTOMER SUPPORT AGENT EVALUATION")
    print("=" * 80)

    baseline_dataset = create_baseline_dataset()
    baseline_dataset.print_execution_plan()
    baseline_results = baseline_dataset.evaluate()

    print("\n📊 Baseline Results:\n")
    baseline_results.as_table(show_tasks=True)

    print("\n" + "=" * 80)
    print("IMPROVED CUSTOMER SUPPORT AGENT EVALUATION")
    print("=" * 80)

    # we only update the agent_response in the context to simulate improved responses
    improved_data = generate_improved_interactions()
    context_map = {}
    for idx, response in enumerate(improved_data):
        context_map[f"support_interaction_{idx}"] = {
            "agent_response": response,
        }

    improved_dataset = baseline_dataset.with_updated_contexts_by_id(context_map)
    improved_results = improved_dataset.evaluate()

    print("\n📊 Improved Results:\n")
    improved_results.as_table(show_tasks=True)

    print("\n" + "=" * 80)
    print("COMPARISON ANALYSIS")
    print("=" * 80)

    comparison = improved_results.compare_to(
        baseline=baseline_results,
        regression_threshold=0.05,
    )

    comparison.as_table()

    if comparison.regressed_workflows > 0:
        print("\n⚠️  REGRESSION DETECTED - Review failed workflows")
    elif comparison.improved_workflows > 0:
        print("\n✅ AGENT IMPROVEMENT CONFIRMED")
    else:
        print("\n➡️  No significant change detected")


if __name__ == "__main__":
    main()
