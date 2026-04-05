# EvalDataset

`EvalDataset` evaluates a set of pre-generated records — no callable agent required. You supply `EvalRecord` objects directly alongside evaluation tasks. Use it when you already have records from a previous run, a production log export, or a data pipeline, and you want to run eval tasks against them in batch.

Use `EvalDataset` when:

- You don't have a callable agent. Only records.
- You're doing post-hoc analysis on production samples.
- You want to run tasks against records that were generated separately from the eval run.

`EvalDataset` doesn't support regression comparison, multi-agent structure, or trace correlation. For those, use [`EvalOrchestrator`](./offline-evaluation.md).

---

## Example: appliance customer service evaluation

This example shows conditional routing across multiple product categories using `condition=True` on `AssertionTask`.

### Step 1: generate records

```python
import random
from typing import List, Literal

from pydantic import BaseModel
from scouter.evaluate import AssertionTask, ComparisonOperator, EvalDataset, LLMJudgeTask
from scouter.genai import Agent, Prompt, Provider
from scouter.queue import EvalRecord

categories = ["bath", "kitchen", "outdoor"]
ApplianceCategory = Literal["kitchen", "bath", "outdoor"]


class UserQuestion(BaseModel):
    question: str
    category: ApplianceCategory


class AgentResponse(BaseModel):
    answer: str
    product_recommendations: List[str]
    safety_notes: List[str]


def simulate_agent_interaction(num_questions: int) -> List[EvalRecord]:
    agent = Agent(Provider.Gemini)

    question_prompt = Prompt(
        messages=(
            "Generate a realistic customer question about one of three appliance "
            "categories: kitchen, bath, or outdoor. Category: ${category}"
        ),
        model="gemini-2.5-flash-lite",
        provider="gemini",
        output_type=UserQuestion,
    )
    response_prompt = Prompt(
        messages=(
            "You are a home appliance expert. Answer this customer question.\n\n"
            "Question: ${user_question}"
        ),
        model="gemini-2.5-flash-lite",
        provider="gemini",
        output_type=AgentResponse,
    )

    records = []
    for _ in range(num_questions):
        category = categories[random.randint(0, 2)]
        question = agent.execute_prompt(
            prompt=question_prompt.bind(category=category),
            output_type=UserQuestion,
        ).structured_output

        response = agent.execute_prompt(
            prompt=response_prompt.bind(user_question=question.question),
            output_type=AgentResponse,
        ).structured_output

        records.append(EvalRecord(context={
            "user_input": question.question,
            "agent_response": response.model_dump_json(),
        }))

    return records
```

### Step 2: define evaluation tasks

```python
from pydantic import BaseModel


class CategoryValidation(BaseModel):
    category: ApplianceCategory
    reason: str
    confidence: float


class KitchenExpertValidation(BaseModel):
    is_suitable: bool
    reason: str
    addresses_safety: bool
    technical_accuracy_score: int


# Base classification task — runs for every record
classification_task = LLMJudgeTask(
    id="category_classification",
    prompt=Prompt(
        messages=(
            "Classify the appliance category (kitchen, bath, outdoor).\n\n"
            "Question: ${user_input}\nResponse: ${agent_response}"
        ),
        model="gemini-2.5-flash-lite",
        provider="gemini",
        output_type=CategoryValidation,
    ),
    expected_value=None,
    operator=ComparisonOperator.IsNotEmpty,
    context_path="category",
)

# Kitchen validation chain — only runs when category_classification.category == "kitchen"
kitchen_tasks = [
    AssertionTask(
        id="is_kitchen_category",
        context_path="category_classification.category",
        operator=ComparisonOperator.Equals,
        expected_value="kitchen",
        depends_on=["category_classification"],
        condition=True,  # gates all downstream kitchen tasks
    ),
    LLMJudgeTask(
        id="kitchen_expert_validation",
        prompt=Prompt(
            messages=(
                "You are a kitchen appliance specialist. Evaluate this response.\n\n"
                "Question: ${user_input}\nResponse: ${agent_response}"
            ),
            model="gemini-2.5-flash-lite",
            provider="gemini",
            output_type=KitchenExpertValidation,
        ),
        expected_value=True,
        operator=ComparisonOperator.Equals,
        context_path="is_suitable",
        depends_on=["is_kitchen_category"],
    ),
    AssertionTask(
        id="kitchen_technical_score",
        context_path="kitchen_expert_validation.technical_accuracy_score",
        operator=ComparisonOperator.GreaterThanOrEqual,
        expected_value=7,
        depends_on=["kitchen_expert_validation"],
    ),
]
# Define bath_tasks and outdoor_tasks following the same pattern
```

### Step 3: assemble and run

```python
records = simulate_agent_interaction(num_questions=10)

dataset = EvalDataset(
    records=records,
    tasks=[classification_task] + kitchen_tasks,  # + bath_tasks + outdoor_tasks
)

dataset.print_execution_plan()
results = dataset.evaluate()
results.as_table()
results.as_table(show_tasks=True)
```

---

## Conditional routing

Tasks with `condition=True` act as gates. When a gate fails, all downstream tasks that depend on it are skipped. No LLM calls are wasted on records that don't match the expected category.

```
category_classification (always runs)
    ├── is_kitchen_category (condition=True) → gates kitchen chain
    │     └── kitchen_expert_validation → kitchen_technical_score
    ├── is_bath_category (condition=True) → gates bath chain
    │     └── bath_expert_validation → bath_installation_score
    └── is_outdoor_category (condition=True) → gates outdoor chain
          └── outdoor_expert_validation → outdoor_durability_score
```

See [Conditional gates](./gates.md) for a full explanation of how gates interact with `depends_on`.

---

## Context flow

Each task only sees its `EvalRecord` base context plus the outputs of tasks it declares in `depends_on`. A task that doesn't declare a dependency cannot access that upstream task's output. This is intentional; it prevents implicit coupling between tasks.

```python
# This task can read category_classification.category
AssertionTask(
    id="is_kitchen_category",
    context_path="category_classification.category",
    depends_on=["category_classification"],  # makes the output available
    ...
)

# This task can read kitchen_expert_validation.technical_accuracy_score
# but NOT category_classification (not in depends_on)
AssertionTask(
    id="kitchen_technical_score",
    context_path="kitchen_expert_validation.technical_accuracy_score",
    depends_on=["kitchen_expert_validation"],
    ...
)
```
