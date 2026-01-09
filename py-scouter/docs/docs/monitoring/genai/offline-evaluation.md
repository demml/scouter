## Offline GenAI Evaluation with Scouter

This guide demonstrates how to perform offline batch evaluations of GenAI services using Scouter's evaluation framework. We'll walk through a real-world example of evaluating a home appliance customer service agent across different product categories.

## Overview

Offline evaluation allows you to test and validate your GenAI service's outputs in batch mode before deploying to production. This is essential for:

- **Regression Testing**: Ensure new model versions maintain quality standards
- **Model Comparison**: Evaluate different models or prompts side-by-side
- **Quality Assurance**: Validate responses meet domain-specific requirements
- **Category-Specific Validation**: Apply specialized evaluation logic based on context

## Key Concepts

## Evaluation Components

Scouter's offline evaluation framework consists of four main building blocks:

- **GenAIEvalRecord**: Contains the context (input/output pairs) to evaluate
- **GenAIEvalDataset**: Collection of records and evaluation tasks
- **LLMJudgeTask**: Uses an LLM to evaluate injected context
- **AssertionTask**: Validates specific conditions without LLM calls
- **Task Dependencies and Conditional Execution**: Tasks can depend on other tasks, creating an evaluation workflow. Use the condition parameter on tasks to create branching logic:

    - When condition=True, the task acts as a conditional gate
    - Downstream tasks only execute if the conditional task passes
    - This enables category-specific validation paths in a single dataset

## Example: Home Appliance Customer Service Evaluation

Let's build an evaluation system for a customer service agent that handles questions about kitchen, bath, and outdoor appliances.

### Step 1: Generate Test Data

First, we simulate customer interactions by generating questions and agent responses. For simplicity, we'll use random generation here, but in practice, you might pull from historical data or a test set. Here we are randomly generating question answer pairs for three categories: kitchen, bath, and outdoor appliances. The questions are designed to be realistic customer inquiries, and the agent responses include answers, product recommendations, and safety notes.

```python

# imports for entire example
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
from scouter.logging import LoggingConfig, LogLevel, RustyLogger
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
            "Generate a customer question for the category: ${category}"
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

def simulate_agent_interaction(num_questions: int) -> List[GenAIEvalRecord]:
    """Generates user questions and agent responses for appliance support.

    Args:
        num_questions (int): Number of question/response pairs to generate.
    Returns:
        List[GenAIEvalRecord]: Generated evaluation records.
    """
    print(f"\n=== Generating {num_questions} Customer Questions ===")
    question_prompt = create_question_generation_prompt()
    user_questions: List[UserQuestion] = []

    # create agent to run prompts
    agent = Agent(Provider.Gemini)

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
```

### Step 2: Define Evaluation Tasks

**Base Classification Task**

The first task classifies which appliance category the question belongs to. The output of this task will determine which specialized evaluation path to follow.
The classification task will do the following:

- Parse the injected context for the `GenAIAvalRecord` (**user_input** and **agent_response**)
- Classify the appliance category (kitchen, bath, outdoor) and provide reasoning with a structured output (`CategoryValidation`)

```python

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
            "${user_input}\n\n" # (1)
            "Agent Response:\n"
            "${agent_response}\n\n" # (2)
            "Classify the category and provide your reasoning:"
        ),
        model="gemini-2.5-flash-lite",
        provider="gemini",
        output_type=CategoryValidation,
)

classification_judge_task = LLMJudgeTask(
    id="category_classification",
    prompt=create_category_classification_prompt(),
    expected_value=None,
    operator=ComparisonOperator.IsNotEmpty,
    field_path="category",
    description="Classify the appliance category"
)
```

1. Injected user question from the record context
2. Injected agent response from the record context

**Category-Specific Validation Chains**

Each category has its own validation chain that only executes if the record belongs to that category:


#### Kitchen Appliance Validation Chain

```python
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

def kitchen_category_tasks() -> List[LLMJudgeTask | AssertionTask]:
    """
    Builds the evaluation tasks specific to the kitchen appliance category.

    Steps:
        1. Conditional check to see if the category is "kitchen"
        2. Expert validation using LLMJudgeTask
        3. Technical accuracy assertion
    """
    return [
        # Conditional check - only proceed if category is "kitchen"
        AssertionTask(
            id="is_kitchen_category",
            field_path="category_classification.category",
            operator=ComparisonOperator.Equals,
            expected_value="kitchen",
            description="Check if categorized as kitchen",
            depends_on=["category_classification"],
            condition=True,  # Only execute kitchen tasks if this passes
        ),
        # Expert validation with LLM
        LLMJudgeTask(
            id="kitchen_expert_validation",
            prompt=create_kitchen_expert_prompt(),
            expected_value=True,
            operator=ComparisonOperator.Equals,
            field_path="is_suitable",
            description="Kitchen expert validates response quality",
            depends_on=["is_kitchen_category"],
        ),
        # Technical accuracy check
        AssertionTask(
            id="kitchen_technical_score",
            field_path="kitchen_expert_validation.technical_accuracy_score",
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=7,
            description="Technical accuracy must be ≥7/10",
            depends_on=["kitchen_expert_validation"],
        ),
    ]
```

#### Batch Appliance Validation Chain

```python
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

def bath_category_tasks() -> List[LLMJudgeTask | AssertionTask]:
    """
    Builds the evaluation tasks specific to the bath appliance category.

    Steps:
        1. Conditional check to see if the category is "bath"
        2. Expert validation using LLMJudgeTask
        3. Installation guidance assertion
    """
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
```

#### Outdoor Appliance Validation Chain

```python

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

def outdoor_category_tasks() -> List[LLMJudgeTask | AssertionTask]:
    """
    Builds the evaluation tasks specific to the outdoor appliance category.

    Steps:
        1. Conditional check to see if the category is "outdoor"
        2. Expert validation using LLMJudgeTask
        3. Durability assessment assertion
    """
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

```

### Step 3: Assemble the Evaluation Dataset

Combine all tasks into a single evaluation dataset:

```python
def build_appliance_support_dataset(
    records: Sequence[GenAIEvalRecord],
) -> GenAIEvalDataset:
    """
    Creates an evaluation dataset for validating appliance support responses.
    """

    dataset = GenAIEvalDataset(
        records=records,
        tasks=[
            # Base classification (always runs)
            classification_judge_task,
        ]
        + kitchen_category_tasks()    # Runs only for kitchen items
        + bath_category_tasks()       # Runs only for bath items
        + outdoor_category_tasks(),   # Runs only for outdoor items
    )
    return dataset
```


### Step 4: Execute the Evaluation

Finally, run the evaluation and review results:

```python

# Generate test records
records = simulate_agent_interaction(num_questions=10)

# Build dataset
dataset = build_appliance_support_dataset(records=records)

# View execution plan
dataset.print_execution_plan()

# Run evaluation
results = dataset.evaluate()

# Display results
results.as_table()              # Workflow Summary view
results.as_table(show_tasks=True)  # Detailed task results
```

#### Understanding Evaluation Flow

For each record, the evaluation follows this flow:

```shell
1. category_classification (LLMJudgeTask, condition=True)
   └─> Determines: kitchen | bath | outdoor
       │
       ├─> If kitchen:
       │   └─> is_kitchen_category (AssertionTask, condition=True)
       │       └─> kitchen_expert_validation (LLMJudgeTask)
       │           └─> kitchen_technical_score (AssertionTask)
       │
       ├─> If bath:
       │   └─> is_bath_category (AssertionTask, condition=True)
       │       └─> bath_expert_validation (LLMJudgeTask)
       │           └─> bath_installation_score (AssertionTask)
       │
       └─> If outdoor:
           └─> is_outdoor_category (AssertionTask, condition=True)
               └─> outdoor_expert_validation (LLMJudgeTask)
                   └─> outdoor_durability_score (AssertionTask)
```

##### Key Points:

- The category_classification task routes evaluation to the appropriate specialist path
- Each category's conditional gate (is_*_category) ensures only relevant validations run
- Tasks with condition=True act as boolean gates - downstream tasks only execute if they pass
- This prevents unnecessary LLM calls and enables efficient category-specific validation

##### Context Flow

Understanding how context flows through evaluation tasks is crucial for building effective evaluation workflows. Let's break down how Scouter manages and propagates context through task dependencies.

**Base Context**

Every evaluation starts with a **base context map** provided in the `GenAIEvalRecord`. In our example, the base context contains two keys:

```python
record = GenAIEvalRecord(
    context={
        "user_input": "What's the best grill for outdoor cooking?",
        "agent_response": '{"answer": "...", "product_recommendations": [...], "safety_notes": [...]}'
    }
)
```

This base context is available to **all tasks** in the evaluation workflow.

**Context Reconstruction Per Task**

For each task execution, the context is **rebuilt** by combining:

1. **Base context** (always included)
2. **Dependency outputs** (outputs from tasks this task depends on)

This means each task gets a fresh context map containing only what it needs.

**Visualization: Context Reconstruction**

Here's how context is rebuilt for each task in our outdoor appliance validation chain:

```
Task 1: category_classification
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Context built for this task:
┌─────────────────────────────────────────┐
│ Base Context                            │
├─────────────────────────────────────────┤
│ user_input: "What's the best grill?"    │
│ agent_response: "{...}"                 │
└─────────────────────────────────────────┘
↓ Executes and produces output
Output: { category: "outdoor", reason: "...", confidence: 0.95 }


Task 2: is_outdoor_category (depends_on: ["category_classification"])
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Context rebuilt for this task:
┌───────────────────────────────────────────────────────┐
│ Base Context + Dependency Context                     │
├───────────────────────────────────────────────────────┤
│ user_input: "What's the best grill?"                  │
│ agent_response: "{...}"                               │
│ category_classification: {                            │  ← Added from dependency
│   category: "outdoor",                                │
│   reason: "Question about grills...",                 │
│   confidence: 0.95                                    │
│ }                                                     │
└───────────────────────────────────────────────────────┘
↓ Reads category_classification.category
↓ Evaluates: "outdoor" == "outdoor" → Pass
Output: True


Task 3: outdoor_expert_validation (depends_on: ["is_outdoor_category"])
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Context rebuilt for this task:
┌───────────────────────────────────────────────────────┐
│ Base Context + Dependency Context                     │
├───────────────────────────────────────────────────────┤
│ user_input: "What's the best grill?"                  │
│ agent_response: "{...}"                               │
│ is_outdoor_category: True                             │  ← Added from dependency
└───────────────────────────────────────────────────────┘
Note: category_classification is NOT included because
      outdoor_expert_validation doesn't depend on it directly

↓ Uses user_input and agent_response in prompt
↓ LLM evaluates the response
Output: {
  is_suitable: true,
  reason: "...",
  weather_considerations: true,
  durability_assessment_score: 8
}


Task 4: outdoor_durability_score (depends_on: ["outdoor_expert_validation"])
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Context rebuilt for this task:
┌───────────────────────────────────────────────────────┐
│ Base Context + Dependency Context                     │
├───────────────────────────────────────────────────────┤
│ user_input: "What's the best grill?"                  │
│ agent_response: "{...}"                               │
│ outdoor_expert_validation: {                          │  ← Added from dependency
│   is_suitable: true,                                  │
│   reason: "...",                                      │
│   weather_considerations: true,                       │
│   durability_assessment_score: 8                      │
│ }                                                     │
└───────────────────────────────────────────────────────┘
Note: is_outdoor_category and category_classification
      are NOT included - only direct dependencies

↓ Reads outdoor_expert_validation.durability_assessment_score
↓ Evaluates: 8 >= 7 → Pass
Output: True
```

**Field Path Resolution**

When a task uses `field_path` to reference upstream data, it's reading from the reconstructed context:

```python
AssertionTask(
    id="outdoor_durability_score",
    field_path="outdoor_expert_validation.durability_assessment_score",
    #          ^^^^^^^^^^^^^^^^^^^^^^^^ ^^^^^^^^^^^^^^^^^^^^^^^^^^^
    #          Dependency task ID       Field in that task's output
    operator=ComparisonOperator.GreaterThanOrEqual,
    expected_value=7,
    depends_on=["outdoor_expert_validation"],  # This makes the output available
)
```

**Key Points**

1. **Explicit Dependencies**: You must declare `depends_on` to access upstream task outputs
2. **Clean Context**: Each task only sees relevant data, not everything that came before
3. **Memory Efficient**: Context doesn't grow unbounded through long chains
4. **Clear Data Flow**: Looking at `depends_on` tells you exactly what data a task can access
