import re

import pytest
from pydantic import BaseModel
from scouter.queue import GenAIRecord


class Context(BaseModel):
    input: str
    response: str


def test_genai_record():
    """
    Test the GenAIRecord class.
    """
    record = GenAIRecord(
        context={
            "input": "What is the capital of France?",
            "response": "Paris is the capital of France.",
        },
    )

    assert record.context["input"] == "What is the capital of France?"
    assert record.context["response"] == "Paris is the capital of France."

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

    record = GenAIRecord(
        context={
            "role": "system",
            "content": system_prompt,
        },
    )

    # Test error - provide no input or response
    with pytest.raises(
        TypeError,
        match=re.escape("GenAIRecord.__new__() missing 1 required positional argument: 'context'"),
    ):
        GenAIRecord()

    record = GenAIRecord(
        context={"foo": "bar", "value": 1},
        prompt=system_prompt,
    )

    # test with pydantic model
    context = Context(
        input="What is the capital of France?",
        response="Paris is the capital of France.",
    )
    record = GenAIRecord(
        context=context,
        prompt=system_prompt,
    )

    assert record.context["input"] == "What is the capital of France?"
    assert record.context["response"] == "Paris is the capital of France."

    # pass incorrect type for context
    with pytest.raises(
        RuntimeError,
        match=re.escape("Invalid context type. Context must be a PyDict or a Pydantic BaseModel"),
    ):
        GenAIRecord(context="This is a string, not a dict or pydantic model")
