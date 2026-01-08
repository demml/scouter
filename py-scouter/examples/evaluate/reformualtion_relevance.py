from scouter.evaluate import (
    AssertionTask,
    ComparisonOperator,
    EvaluationConfig,
    GenAIEvalDataset,
    LLMJudgeTask,
)
from scouter.genai import Embedder, Prompt, Provider, Score
from scouter.genai.openai import OpenAIEmbeddingConfig
from scouter.logging import LoggingConfig, LogLevel, RustyLogger
from scouter.queue import GenAIEvalRecord

RustyLogger.setup_logging(LoggingConfig(log_level=LogLevel.Debug))

# Define evaluation prompt for reformulation quality
reformulation_eval_prompt = Prompt(
    messages=(
        "You are an expert evaluator of search query relevance. \n"
        "You will be given a user query and its reformulated version. \n"
        "Your task is to assess how relevant the reformulated query is to the information needs of the user. \n"
        "Consider the following criteria:\n"
        "- Does the query contain relevant keywords and concepts?\n"
        "- Is the query clear and unambiguous?\n"
        "- Does the query adequately express the user's intent?\n\n"
        "Provide your evaluation as a JSON object with the following attributes:\n"
        "- score: An integer from 1 (poor) to 5 (excellent) indicating the overall reformulation score.\n"
        "- reason: A brief explanation for your score.\n\n"
        "Format your response as:\n"
        "{\n"
        '  "score": <integer 1-5>,\n'
        '  "reason": "<your explanation>"\n'
        "}\n\n"
        "User Query:\n"
        "${user_query}\n\n"
        "Reformulated Query:\n"
        "${reformulated_query}\n\n"
        "Evaluation:"
    ),
    model="gemini-2.5-flash-lite",
    provider="gemini",
    output_type=Score,
)

# Define evaluation prompt for answer relevance
answer_eval_prompt = Prompt(
    messages=(
        "You are an expert evaluator of answer relevance. \n"
        "You will be given a user query and an answer generated from a reformulated version of that query. \n"
        "Your task is to assess how relevant and accurate the answer is in addressing the user's original information needs. \n"
        "Consider the following criteria:\n"
        "- Does the answer directly address the user's query?\n"
        "- Is the information provided accurate and reliable?\n"
        "- Is the answer clear, concise, and well-structured?\n\n"
        "Provide your evaluation as a JSON object with the following attributes:\n"
        "- score: An integer from 1 (poor) to 5 (excellent) indicating the overall answer quality score.\n"
        "- reason: A brief explanation for your score.\n\n"
        "Format your response as:\n"
        "{\n"
        '  "score": <integer 1-5>,\n'
        '  "reason": "<your explanation>"\n'
        "}\n\n"
        "User Query:\n"
        "${user_query}\n\n"
        "Answer:\n"
        "${answer}\n\n"
        "Evaluation:"
    ),
    model="gemini-2.5-flash-lite",
    provider="gemini",
    output_type=Score,
)

# Create evaluation records
flight_record = GenAIEvalRecord(
    context={
        "user_query": "cheap flights to Europe next month",
        "reformulated_query": "affordable airfare to Europe next month",
        "answer": "I found several options for cheap flights to Europe next month.",
    },
)

technical_record = GenAIEvalRecord(
    context={
        "user_query": "why won't my laptop turn on",
        "reformulated_query": "laptop computer won't boot power issues troubleshooting steps hardware failure battery power supply diagnostic repair",
        "answer": "If your laptop won't turn on, try these troubleshooting steps: 1) Check power connections - ensure the charger is plugged in securely and the power outlet works. 2) Remove the battery (if removable) and hold the power button for 30 seconds, then reconnect and try again. 3) Look for LED indicators on the laptop or charger. 4) Try a different power adapter if available. 5) Check for physical damage to ports or cables. 6) If these steps don't work, the issue may be hardware-related (motherboard, RAM, or hard drive failure) requiring professional repair",
    },
)

cooking_record = GenAIEvalRecord(
    context={
        "user_query": "easy dinner recipes with chicken",
        "reformulated_query": "simple quick chicken dinner recipes healthy family-friendly weeknight meals",
        "answer": "Here are some easy chicken dinner recipes: 1) Baked Lemon Garlic Chicken - Marinate chicken breasts in lemon juice, garlic, olive oil, and herbs, then bake until cooked through. 2) One-Pan Chicken and Veggies - Sauté chicken pieces with mixed vegetables in a skillet with olive oil and your favorite seasonings. 3) Chicken Stir-Fry - Cook sliced chicken with colorful veggies in a wok or large pan, adding soy sauce and ginger for flavor. 4) Chicken Tacos - Season shredded chicken with taco seasoning and serve in tortillas with your favorite toppings. 5) Chicken Alfredo Pasta - Toss cooked pasta with grilled chicken and a creamy Alfredo sauce for a quick and satisfying meal.",
    },
)

# Create the evaluation dataset with tasks
dataset = GenAIEvalDataset(
    records=[flight_record, technical_record, cooking_record],
    tasks=[
        LLMJudgeTask(
            id="reformulation_quality",
            prompt=reformulation_eval_prompt,
            expected_value=3,
            operator=ComparisonOperator.GreaterThanOrEqual,
            field_path="score",
            description="Evaluate the quality of query reformulation",
        ),
        LLMJudgeTask(
            id="answer_relevance",
            prompt=answer_eval_prompt,
            expected_value=3,
            operator=ComparisonOperator.GreaterThanOrEqual,
            field_path="score",
            description="Evaluate the relevance of the answer to the user query",
        ),
        AssertionTask(
            id="not_empty_reformulation",
            expected_value=True,
            operator=ComparisonOperator.IsNotEmpty,
            field_path="answer",
            description="Check that the answer is not empty",
        ),
    ],
)

embedder = Embedder(
    Provider.OpenAI,
    config=OpenAIEmbeddingConfig(
        model="text-embedding-3-small",
        dimensions=512,
    ),
)

# show execution plan
dataset.print_execution_plan()

# Run the evaluation
results = dataset.evaluate(
    config=EvaluationConfig(
        embedder=embedder,
        embedding_targets=["user_query", "answer"],
        compute_similarity=True,
        compute_histograms=True,
    ),
)

# show worflow summary table
results.as_table(show_tasks=True)

print(results.to_dataframe().head())
