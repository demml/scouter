### The following example shows how to compute drift from a list of LLM records using the `compute_drift` method in python
### You would typically let the server handle this, but this is to demonstrate the functionality.

from scouter.alert import AlertThreshold
from scouter.drift import Drifter, LLMDriftConfig, LLMDriftProfile, LLMMetric
from scouter.llm import Agent, Prompt, Provider, Score
from scouter.logging import LoggingConfig, LogLevel, RustyLogger
from scouter.queue import LLMRecord

RustyLogger.setup_logging(LoggingConfig(log_level=LogLevel.Debug))


def create_reformulation_evaluation_prompt():
    """
    Builds a prompt for evaluating the quality of a reformulated query.

    Returns:
        Prompt: A prompt that asks for a JSON evaluation of the reformulation.

    Example:
        >>> prompt = create_reformulation_evaluation_prompt()
    """
    return Prompt(
        user_message=(
            "You are an expert evaluator of search query reformulations. "
            "Given the original user query and its reformulated version, your task is to assess how well the reformulation improves the query. "
            "Consider the following criteria:\n"
            "- Does the reformulation make the query more explicit and comprehensive?\n"
            "- Are relevant synonyms, related concepts, or specific features added?\n"
            "- Is the original intent preserved without changing the meaning?\n"
            "- Is the reformulation clear and unambiguous?\n\n"
            "Provide your evaluation as a JSON object with the following attributes:\n"
            "- score: An integer from 1 (poor) to 5 (excellent) indicating the overall quality of the reformulation.\n"
            "- reason: A brief explanation for your score.\n\n"
            "Format your response as:\n"
            "{\n"
            '  "score": <integer 1-5>,\n'
            '  "reason": "<your explanation>"\n'
            "}\n\n"
            "Original Query:\n"
            "${user_query}\n\n"
            "Reformulated Query:\n"
            "${response}\n\n"
            "Evaluation:"
        ),
        model="gemini-2.5-flash-lite-preview-06-17",
        provider="gemini",
        response_format=Score,
    )


def create_query_reformulation_prompt():
    """Builds a prompt for query reformulation tasks."""
    return Prompt(
        user_message=(
            "You are an expert at query reformulation. Your task is to take a user's original search query "
            "and rewrite it to be more feature-rich and keyword-dense, so it better aligns with the user's intent "
            "and improves search results.\n\n"
            "Guidelines:\n"
            "- Expand abbreviations and clarify ambiguous terms.\n"
            "- Add relevant synonyms, related concepts, and specific features.\n"
            "- Preserve the original intent, but make the query more explicit and comprehensive.\n"
            "- Do not change the meaning of the query.\n"
            "- Return only the reformulated query.\n\n"
            "User Query:\n"
            "${user_query}\n\n"
            "Reformulated Query:"
        ),
        model="gemini-2.5-flash-lite-preview-06-17",
        provider="gemini",
    )


def create_llm_drift_profile() -> LLMDriftProfile:
    """Helper function to create a LLM drift profile for query reformulation tasks."""
    eval_prompt = create_reformulation_evaluation_prompt()
    profile = LLMDriftProfile(
        config=LLMDriftConfig(),
        metrics=[
            LLMMetric(
                name="reformulation_quality",
                prompt=eval_prompt,
                value=3.0,
                alert_threshold=AlertThreshold.Below,
            )
        ],
    )

    return profile


# Example usage
prompt = create_query_reformulation_prompt()
agent = Agent(Provider.Gemini)


if __name__ == "__main__":
    user_query = "How do I find good post-hardcore bands?"
    response = agent.execute_prompt(prompt=prompt.bind(user_query=user_query))

    profile = create_llm_drift_profile()
    record = LLMRecord(
        context={"user_query": user_query, "response": response.result},
    )

    drifter = Drifter()
    results = drifter.compute_drift(record, profile)

    print(results)
