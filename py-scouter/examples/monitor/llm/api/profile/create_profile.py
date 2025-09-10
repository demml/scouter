from pathlib import Path

from scouter.alert import AlertThreshold
from scouter.client import ScouterClient
from scouter.drift import LLMDriftConfig, LLMDriftMetric, LLMDriftProfile
from scouter.llm import Prompt, Score


def create_reformulation_evaluation_prompt():
    """
    Builds a prompt for evaluating the quality of a reformulated query.

    Returns:
        Prompt: A prompt that asks for a JSON evaluation of the reformulation.

    Example:
        >>> prompt = create_reformulation_evaluation_prompt()
    """
    return Prompt(
        message=(
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
            "${user_input}\n\n"
            "Reformulated Query:\n"
            "${reformulated_query}\n\n"
            "Evaluation:"
        ),
        model="gemini-2.5-flash-lite-preview-06-17",
        provider="gemini",
        response_format=Score,
    )


def create_relevance_evaluation_prompt() -> Prompt:
    """
    Builds a prompt for evaluating the relevance of an LLM response to the original user query.

    Returns:
        Prompt: A prompt that asks for a JSON evaluation of the response's relevance.

    Example:
        >>> prompt = create_relevance_evaluation_prompt()
    """
    return Prompt(
        message=(
            "You are an expert evaluator of LLM responses. "
            "Given the original user query and the LLM's response, your task is to assess how relevant the response is to the query. "
            "Consider the following criteria:\n"
            "- Does the response directly address the user's question or request?\n"
            "- Is the information provided accurate and appropriate for the query?\n"
            "- Are any parts of the response off-topic or unrelated?\n"
            "- Is the response complete and does it avoid unnecessary information?\n\n"
            "Provide your evaluation as a JSON object with the following attributes:\n"
            "- score: An integer from 1 (irrelevant) to 5 (highly relevant) indicating the overall relevance of the response.\n"
            "- reason: A brief explanation for your score.\n\n"
            "Format your response as:\n"
            "{\n"
            '  "score": <integer 1-5>,\n'
            '  "reason": "<your explanation>"\n'
            "}\n\n"
            "Original Query:\n"
            "${reformulated_query}\n\n"
            "LLM Response:\n"
            "${relevance_response}\n\n"
            "Evaluation:"
        ),
        model="gemini-2.5-flash-lite-preview-06-17",
        provider="gemini",
        response_format=Score,
    )


relevance = LLMDriftMetric(
    name="relevance",
    prompt=create_relevance_evaluation_prompt(),
    value=5.0,
    alert_threshold_value=2.0,
    alert_threshold=AlertThreshold.Below,
)
reformulation = LLMDriftMetric(
    name="reformulation",
    prompt=create_reformulation_evaluation_prompt(),
    value=5.0,
    alert_threshold_value=2.0,
    alert_threshold=AlertThreshold.Above,
)

profile = LLMDriftProfile(
    config=LLMDriftConfig(
        space="scouter",
        name="llm_metrics",
        version="0.0.1",
        sample_rate=1,
    ),
    metrics=[relevance, reformulation],
)


if __name__ == "__main__":
    # Create a LLM drift profile and register it with the Scouter client
    # This assumes that the Scouter client is running and configured correctly
    client = ScouterClient()
    client.register_profile(profile=profile, set_active=True)

    profile.save_to_json(Path("api/assets/llm_drift_profile.json"))
