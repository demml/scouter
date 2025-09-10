## LLM Evaluation

In addition to real-time or online monitoring of LLMs, Scouter provides you with tools to run offline LLM evaluations. This is is often useful for when you (1) want to compare and benchmark various prompts, and (2) you want to evaluate different versions of prompts and LLM services that you may already be using in production.

## Getting Started

To run and LLM evaluation, you will first need to obtain your evaluation data and construct a list of `LLMEvalRecord` instances. Each `LLMEvalRecord` represents a single evaluation instance, containing the metadata you wish to evaluate. Note, we have left this intentionally flexible so that you can evaluate any type of metadata you wish.

### Example: Creating Evaluation Records

Let's say you have a use case where you want to evaluate how well a prompt reformulates user search queries. The prompt used for this task is shown below and does the following things:

- Takes a bound parameter `${user_query}` which is the original user search query.
- Reformulates the query to be more feature-rich and keyword-dense, while preserving the original

```python
Prompt(
    message=(
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
    model="gemini-2.5-flash-lite",
    provider="gemini",
    model_settings=GeminiSettings(
        generation_config=GenerationConfig(
            thinking_config=ThinkingConfig(thinking_budget=0),
        ),
    ),
)
```

This prompt is a reformulation prompt th