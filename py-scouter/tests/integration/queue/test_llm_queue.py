import pytest
from scouter.queue import LLMRecord


def test_llm_record():
    """
    Test the LLMRecord class.
    """
    record = LLMRecord(
        input="What is the capital of France?",
        response="Paris is the capital of France.",
    )

    assert record.input == "What is the capital of France?"
    assert record.response == "Paris is the capital of France."

    # instantiate with list of messages
    system_prompt = {
        "system": """You are a technical expert. Provide detailed, accurate technical explanations.
                Focus on implementation details, best practices, and practical solutions.""",
        "follow_ups": [
            "Can you show me a code example?",
            "What are the potential pitfalls?",
            "How does this scale in production?",
        ],
    }
    messages = [{"role": "system", "content": system_prompt}]
    record = LLMRecord(input=messages)

    # Test error - provide no input or response
    with pytest.raises(
        RuntimeError,
        match="Failed to supply either input or response for the llm record",
    ):
        LLMRecord()

    record = LLMRecord(
        input=messages,
        context={"foo": "bar", "value": 1},
        prompt=system_prompt,
    )
