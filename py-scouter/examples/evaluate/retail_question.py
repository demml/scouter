import random
from typing import List, Literal, Sequence

from pydantic import BaseModel
from scouter.evaluate import (
    AssertionTask,
    ComparisonOperator,
    GenAIEvalDataset,
    LLMJudgeTask,
)
from scouter.genai import Agent, Prompt, Provider
from scouter.queue import GenAIEvalRecord

categories = ["bath", "kitchen", "outdoor"]
ApplianceCategory = Literal["kitchen", "bath", "outdoor"]


class UserQuestion(BaseModel):
    question: str
    category: ApplianceCategory


class AgentResponse(BaseModel):
    answer: str
    product_recommendations: List[str]
    safety_notes: List[str]


class CategoryValidation(BaseModel):
    category: ApplianceCategory
    reason: str
    confidence: float


class KitchenExpertValidation(BaseModel):
    is_suitable: bool
    reason: str
    addresses_safety: bool
    technical_accuracy_score: int


class BathExpertValidation(BaseModel):
    is_suitable: bool
    reason: str
    water_safety_addressed: bool
    installation_guidance_score: int


class OutdoorExpertValidation(BaseModel):
    is_suitable: bool
    reason: str
    weather_considerations: bool
    durability_assessment_score: int


def create_question_generation_prompt() -> Prompt:
    """
    Builds a prompt for generating a random appliance-related user question.
    """
    return Prompt(
        messages=(
            "You are a customer service bot for a home retail company focusing on bath, kitchen, and outdoor. Generate a realistic "
            "customer question about one of three appliance categories: kitchen, bath, or outdoor.\n\n"
            "Guidelines:\n"
            "- Randomly select one category: kitchen (refrigerators, ovens, dishwashers, microwaves), "
            "bath (water heaters, bathroom fans, towel warmers), or outdoor (grills, patio heaters, "
            "outdoor lighting, pool equipment)\n"
            "- Create a specific, detailed question a real customer might ask\n"
            "- Question should be practical and answerable by a knowledgeable agent\n"
            "- Include enough context to make the question realistic\n\n"
            "Generate a customer question: based on the select input ${category}"
        ),
        model="gemini-2.5-flash-lite",
        provider="gemini",
        output_type=UserQuestion,
    )


def create_agent_response_prompt() -> Prompt:
    """
    Builds a prompt for answering the user's appliance question.
    """
    return Prompt(
        messages=(
            "You are a knowledgeable home appliance expert providing customer support. "
            "Answer the following customer question with accurate, helpful information.\n\n"
            "Guidelines:\n"
            "- Provide a clear, detailed answer to the customer's question\n"
            "- Recommend 2-3 specific products or solutions when appropriate\n"
            "- Include any relevant safety notes or warnings\n"
            "- Be professional and friendly\n"
            "- Use technical terms accurately while remaining accessible\n\n"
            "Customer Question:\n"
            "${user_question}\n\n"
            "Provide your response:"
        ),
        model="gemini-2.5-flash-lite",
        provider="gemini",
        output_type=AgentResponse,
    )


def create_category_classification_prompt() -> Prompt:
    """
    Builds a prompt for classifying the appliance category of the user question.
    """
    return Prompt(
        messages=(
            "You are an expert in appliance classification. Analyze the user question and agent response "
            "to determine which appliance category it belongs to.\n\n"
            "Categories:\n"
            "- kitchen: refrigerators, ovens, dishwashers, microwaves, small kitchen appliances\n"
            "- bath: water heaters, bathroom fans, towel warmers, bathroom lighting\n"
            "- outdoor: grills, patio heaters, outdoor lighting, pool equipment, lawn equipment\n\n"
            "User Question:\n"
            "${user_input}\n\n"
            "Agent Response:\n"
            "${agent_response}\n\n"
            "Classify the category and provide your reasoning:"
        ),
        model="gemini-2.5-flash-lite",
        provider="gemini",
        output_type=CategoryValidation,
    )


def create_kitchen_expert_prompt() -> Prompt:
    """
    Builds a prompt for a kitchen appliance expert to evaluate the response.
    """
    return Prompt(
        messages=(
            "You are a certified kitchen appliance specialist with 15 years of experience in "
            "residential kitchen equipment. Evaluate whether the agent's response is suitable "
            "and accurate for this kitchen appliance question.\n\n"
            "Evaluation Criteria:\n"
            "- Technical accuracy of information provided\n"
            "- Appropriate product recommendations for kitchen use\n"
            "- Safety considerations (electrical, fire, food safety)\n"
            "- Proper installation and maintenance guidance\n\n"
            "User Question:\n"
            "${user_input}\n\n"
            "Agent Response:\n"
            "${agent_response}\n\n"
            "Provide your expert evaluation with:\n"
            "- is_suitable: boolean indicating if response is appropriate\n"
            "- reason: detailed explanation of your assessment\n"
            "- addresses_safety: whether critical safety information is included\n"
            "- technical_accuracy_score: score from 1-10 on technical correctness\n\n"
            "Evaluation:"
        ),
        model="gemini-2.5-flash-lite",
        provider="gemini",
        output_type=KitchenExpertValidation,
    )


def create_bath_expert_prompt() -> Prompt:
    """
    Builds a prompt for a bathroom appliance expert to evaluate the response.
    """
    return Prompt(
        messages=(
            "You are a licensed plumber and bathroom fixture specialist with expertise in "
            "water heaters, ventilation, and bathroom electrical systems. Evaluate whether "
            "the agent's response is suitable and accurate for this bath appliance question.\n\n"
            "Evaluation Criteria:\n"
            "- Water safety and plumbing code compliance\n"
            "- Electrical safety for bathroom environments\n"
            "- Proper ventilation considerations\n"
            "- Installation guidance and professional service recommendations\n\n"
            "User Question:\n"
            "${user_input}\n\n"
            "Agent Response:\n"
            "${agent_response}\n\n"
            "Provide your expert evaluation with:\n"
            "- is_suitable: boolean indicating if response is appropriate\n"
            "- reason: detailed explanation of your assessment\n"
            "- water_safety_addressed: whether water/moisture safety is properly covered\n"
            "- installation_guidance_score: score from 1-10 on installation advice quality\n\n"
            "Evaluation:"
        ),
        model="gemini-2.5-flash-lite",
        provider="gemini",
        output_type=BathExpertValidation,
    )


def create_outdoor_expert_prompt() -> Prompt:
    """
    Builds a prompt for an outdoor appliance expert to evaluate the response.
    """
    return Prompt(
        messages=(
            "You are an outdoor living specialist with expertise in grills, patio equipment, "
            "and weatherproof electrical systems. Evaluate whether the agent's response is "
            "suitable and accurate for this outdoor appliance question.\n\n"
            "Evaluation Criteria:\n"
            "- Weather resistance and durability considerations\n"
            "- Proper outdoor electrical safety (GFCI, weatherproofing)\n"
            "- Material recommendations for outdoor environments\n"
            "- Maintenance requirements for outdoor conditions\n\n"
            "User Question:\n"
            "${user_input}\n\n"
            "Agent Response:\n"
            "${agent_response}\n\n"
            "Provide your expert evaluation with:\n"
            "- is_suitable: boolean indicating if response is appropriate\n"
            "- reason: detailed explanation of your assessment\n"
            "- weather_considerations: whether weatherproofing is adequately addressed\n"
            "- durability_assessment_score: score from 1-10 on durability guidance\n\n"
            "Evaluation:"
        ),
        model="gemini-2.5-flash-lite",
        provider="gemini",
        output_type=OutdoorExpertValidation,
    )


def simulate_agent_interaction(
    num_questions: int,
) -> List[GenAIEvalRecord]:
    """Generates user questions and agent responses for appliance support."""
    print(f"\n=== Generating {num_questions} Customer Questions ===")
    question_prompt = create_question_generation_prompt()
    user_questions: List[UserQuestion] = []

    for i in range(num_questions):
        random_int = random.randint(0, 2)
        question = agent.execute_prompt(
            prompt=question_prompt.bind(category=categories[random_int]),
            output_type=UserQuestion,
        ).structured_output
        user_questions.append(question)
        print(f"\nQuestion {i + 1} [{question.category}]:")
        print(f"  {question.question}")

    print("\n=== Generating Agent Responses ===")
    response_prompt = create_agent_response_prompt()
    agent_responses: List[AgentResponse] = []

    for i, question in enumerate(user_questions):
        response = agent.execute_prompt(
            prompt=response_prompt.bind(user_question=question.question),
            output_type=AgentResponse,
        ).structured_output
        agent_responses.append(response)
        print(f"\nResponse {i + 1}:")
        print(f"  Answer: {response.answer[:100]}...")
        print(f"  Products: {', '.join(response.product_recommendations)}")
        print(f"  Safety Notes: {len(response.safety_notes)} items")

    records = []
    for question, response in zip(user_questions, agent_responses):
        record = GenAIEvalRecord(
            context={
                "user_input": question.question,
                "agent_response": response.model_dump_json(),
            }
        )
        records.append(record)

    return records


def kitchen_category_tasks() -> List[LLMJudgeTask | AssertionTask]:
    return [
        AssertionTask(
            id="is_kitchen_category",
            field_path="category_classification.category",
            operator=ComparisonOperator.Equals,
            expected_value="kitchen",
            description="Check if categorized as kitchen",
            depends_on=["category_classification"],
            condition=True,
        ),
        LLMJudgeTask(
            id="kitchen_expert_validation",
            prompt=create_kitchen_expert_prompt(),
            expected_value=True,
            operator=ComparisonOperator.Equals,
            field_path="is_suitable",
            description="Kitchen expert validates response quality and safety",
            depends_on=["is_kitchen_category"],
        ),
        AssertionTask(
            id="kitchen_technical_score",
            field_path="kitchen_expert_validation.technical_accuracy_score",
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=7,
            description="Verify kitchen technical accuracy score is at least 7/10",
            depends_on=["kitchen_expert_validation"],
        ),
    ]


def bath_category_tasks() -> List[LLMJudgeTask | AssertionTask]:
    return [
        AssertionTask(
            id="is_bath_category",
            field_path="category_classification.category",
            operator=ComparisonOperator.Equals,
            expected_value="bath",
            description="Check if categorized as bath",
            depends_on=["category_classification"],
            condition=True,
        ),
        LLMJudgeTask(
            id="bath_expert_validation",
            prompt=create_bath_expert_prompt(),
            expected_value=True,
            operator=ComparisonOperator.Equals,
            field_path="is_suitable",
            description="Bath expert validates response quality and safety",
            depends_on=["is_bath_category"],
        ),
        AssertionTask(
            id="bath_installation_score",
            field_path="bath_expert_validation.installation_guidance_score",
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=7,
            description="Verify bath installation guidance score is at least 7/10",
            depends_on=["bath_expert_validation"],
        ),
    ]


def outdoor_category_tasks() -> List[LLMJudgeTask | AssertionTask]:
    return [
        AssertionTask(
            id="is_outdoor_category",
            field_path="category_classification.category",  # reference upstream field value
            operator=ComparisonOperator.Equals,
            expected_value="outdoor",
            description="Check if categorized as outdoor",
            depends_on=["category_classification"],
            condition=True,
        ),
        LLMJudgeTask(
            id="outdoor_expert_validation",
            prompt=create_outdoor_expert_prompt(),
            expected_value=True,
            operator=ComparisonOperator.Equals,
            field_path="is_suitable",
            description="Outdoor expert validates response quality and safety",
            depends_on=["is_outdoor_category"],
        ),
        AssertionTask(
            id="outdoor_durability_score",
            field_path="outdoor_expert_validation.durability_assessment_score",
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=7,
            description="Verify outdoor durability assessment score is at least 7/10",
            depends_on=["outdoor_expert_validation"],
        ),
    ]


def build_appliance_support_dataset(
    records: Sequence[GenAIEvalRecord],
) -> GenAIEvalDataset:
    """
    Creates an evaluation dataset for validating appliance support responses.
    """

    dataset = GenAIEvalDataset(
        records=records,
        tasks=[
            LLMJudgeTask(
                id="category_classification",
                prompt=create_category_classification_prompt(),
                expected_value=None,
                operator=ComparisonOperator.IsNotEmpty,
                field_path="category",
                description="Classify the appliance category (kitchen, bath, outdoor)",
            ),
        ]
        + kitchen_category_tasks()
        + bath_category_tasks()
        + outdoor_category_tasks(),
    )
    return dataset


if __name__ == "__main__":
    agent = Agent(Provider.Gemini)
    num_questions = 2

    records = simulate_agent_interaction(num_questions=num_questions)

    dataset = build_appliance_support_dataset(records=records)

    print("\n=== Evaluation Plan ===")
    dataset.print_execution_plan()

    print("\n=== Running Evaluation ===")
    results = dataset.evaluate()
    results.as_table()
    results.as_table(show_tasks=True)
