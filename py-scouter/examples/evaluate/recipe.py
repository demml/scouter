from typing import List, Optional

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

RustyLogger.setup_logging(LoggingConfig(log_level=LogLevel.Info))


class Ingredient(BaseModel):
    name: str
    quantity: str
    unit: str


class Rating(BaseModel):
    score: int
    reason: str


class Recipe(BaseModel):
    name: str
    ingredients: List[Ingredient]
    directions: List[str]
    prep_time_minutes: int
    servings: int
    rating: Optional[Rating] = None


class VegetarianValidation(BaseModel):
    is_vegetarian: bool
    reason: str
    non_vegetarian_ingredients: List[str]


def create_recipe_generation_prompt() -> Prompt:
    """
    Builds a prompt for generating a vegetarian recipe with structured output.
    """
    return Prompt(
        messages=(
            "You are an expert chef specializing in vegetarian cuisine. Your task is to create "
            "a complete vegetarian recipe based on the user's request.\n\n"
            "Guidelines:\n"
            "- Ensure all ingredients are vegetarian (no meat, poultry, or seafood)\n"
            "- Provide specific quantities and units for each ingredient\n"
            "- Include detailed, step-by-step cooking directions\n"
            "- Specify prep time (less than 120 minutes) and number of servings\n"
            "- Make the recipe practical and easy to follow\n\n"
            "User Request:\n"
            "${user_request}\n\n"
            "Generate a complete recipe:"
        ),
        model="gemini-2.5-flash-lite",
        provider="gemini",
        output_type=Recipe,
    )


def create_vegetarian_validation_prompt() -> Prompt:
    """
    Builds a prompt for validating that a recipe is truly vegetarian.
    """
    return Prompt(
        messages=(
            "You are a nutrition expert specializing in dietary restrictions. Your task is to verify "
            "whether a recipe is truly vegetarian.\n\n"
            "A vegetarian recipe:\n"
            "- Contains NO meat (beef, pork, lamb, etc.)\n"
            "- Contains NO poultry (chicken, turkey, duck, etc.)\n"
            "- Contains NO seafood (fish, shrimp, shellfish, etc.)\n"
            "- MAY contain eggs, dairy, and honey\n"
            "- MAY contain plant-based meat alternatives\n\n"
            "Analyze the following recipe and determine if it meets vegetarian standards.\n\n"
            "Recipe:\n"
            "${response}\n\n"
            "Provide your evaluation as a JSON object with:\n"
            "- is_vegetarian: boolean indicating if the recipe is vegetarian\n"
            "- reason: explanation for your determination\n"
            "- non_vegetarian_ingredients: list of any non-vegetarian ingredients found (empty list if none)\n\n"
            "Evaluation:"
        ),
        model="gemini-2.5-flash-lite",
        provider="gemini",
        output_type=VegetarianValidation,
    )


def build_recipe_eval_dataset(
    user_request: str, recipe_response: Recipe
) -> GenAIEvalDataset:
    """
    Creates an evaluation dataset for validating vegetarian recipe generation.
    """

    # create 4 records with the same context
    # every other record add rating to the recipe response
    records = []
    for i in range(4):
        if i % 2 == 0:
            response = recipe_response
            response.rating = None
        else:
            response = recipe_response.model_copy()
            response.rating = Rating(score=5, reason="Excellent recipe")

        record = GenAIEvalRecord(
            context={"user_request": user_request, "recipe": response}
        )
        records.append(record)

    dataset = GenAIEvalDataset(
        records=records,
        tasks=[
            LLMJudgeTask(  # LLM judges validate the prompt outputs, not original context
                id="vegetarian_validation",
                prompt=create_vegetarian_validation_prompt(),
                expected_value=True,
                operator=ComparisonOperator.Equals,
                field_path="is_vegetarian",
                description="Validate that the recipe is truly vegetarian",
            ),
            AssertionTask(
                id="has_ingredients",
                field_path="recipe.ingredients",
                operator=ComparisonOperator.HasLengthGreaterThan,
                expected_value=0,
                description="Verify the recipe contains at least one ingredient",
            ),
            AssertionTask(
                id="has_directions",
                field_path="recipe.directions",
                operator=ComparisonOperator.HasLengthGreaterThan,
                expected_value=0,
                description="Verify the recipe contains cooking directions",
            ),
            AssertionTask(
                id="has_valid_servings",
                field_path="recipe.servings",
                operator=ComparisonOperator.GreaterThan,
                expected_value=0,
                description="Verify servings count is greater than zero",
            ),
            AssertionTask(
                id="has_valid_prep_time",
                field_path="recipe.prep_time_minutes",
                operator=ComparisonOperator.InRange,
                expected_value=[0, 120],
                description="Verify prep time is within a reasonable range",
            ),
            # Conditional checks to validate rating if present
            AssertionTask(
                id="has_rating",
                field_path="recipe.rating",
                operator=ComparisonOperator.IsNotEmpty,
                expected_value=True,
                description="Check that the recipe has a rating",
                condition=True,
            ),
            AssertionTask(
                id="valid_rating_score",
                field_path="context.recipe.rating.score",
                operator=ComparisonOperator.InRange,
                expected_value=[1, 5],
                description="Verify rating score is between 1 and 5",
                depends_on=["has_rating"],
            ),
        ],
    )
    return dataset


if __name__ == "__main__":
    user_request = "Create a hearty vegetarian pasta dish with seasonal vegetables"

    prompt = create_recipe_generation_prompt()
    agent = Agent(Provider.Gemini)

    recipe: Recipe = agent.execute_prompt(
        prompt=prompt.bind(user_request=user_request),
        output_type=Recipe,
    ).structured_output

    print("\n=== Generated Recipe ===")
    print(f"Name: {recipe.name}")
    print(f"Prep Time: {recipe.prep_time_minutes} minutes")
    print(f"Servings: {recipe.servings}")
    print(f"\nIngredients ({len(recipe.ingredients)}):")
    for ing in recipe.ingredients:
        print(f"  - {ing.quantity} {ing.unit} {ing.name}")
    print(f"\nDirections ({len(recipe.directions)} steps):")
    for i, direction in enumerate(recipe.directions, 1):
        print(f"  {i}. {direction}")

    dataset = build_recipe_eval_dataset(
        user_request=user_request, recipe_response=recipe
    )

    print("\n=== Evaluation Plan ===")
    dataset.print_execution_plan()

    print("\n=== Running Evaluation ===")
    results = dataset.evaluate()
    results.as_table()
