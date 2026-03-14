from scouter.evaluate import (
    AgentAssertion,
    AgentAssertionTask,
    ComparisonOperator,
    EvalDataset,
    EvalRecord,
    execute_agent_assertion_tasks,
)


def test_tool_called():
    assertion = AgentAssertion.tool_called("web_search")
    assert isinstance(assertion, AgentAssertion.ToolCalled)
    assert assertion.name == "web_search"


def test_tool_not_called():
    assertion = AgentAssertion.tool_not_called("delete_user")
    assert isinstance(assertion, AgentAssertion.ToolNotCalled)
    assert assertion.name == "delete_user"


def test_tool_called_with_args():
    assertion = AgentAssertion.tool_called_with_args("web_search", {"query": "test"})
    assert isinstance(assertion, AgentAssertion.ToolCalledWithArgs)
    assert assertion.name == "web_search"
    assert assertion.arguments == {"query": "test"}


def test_tool_call_sequence():
    names = ["web_search", "summarize", "respond"]
    assertion = AgentAssertion.tool_call_sequence(names)
    assert isinstance(assertion, AgentAssertion.ToolCallSequence)
    assert assertion.names == names


def test_tool_call_count():
    assertion = AgentAssertion.tool_call_count("web_search")
    assert isinstance(assertion, AgentAssertion.ToolCallCount)
    assert assertion.name == "web_search"


def test_tool_call_count_all():
    assertion = AgentAssertion.tool_call_count()
    assert isinstance(assertion, AgentAssertion.ToolCallCount)
    assert assertion.name is None


def test_tool_argument():
    assertion = AgentAssertion.tool_argument("web_search", "query")
    assert isinstance(assertion, AgentAssertion.ToolArgument)
    assert assertion.name == "web_search"
    assert assertion.argument_key == "query"


def test_tool_result():
    assertion = AgentAssertion.tool_result("calculator")
    assert isinstance(assertion, AgentAssertion.ToolResult)
    assert assertion.name == "calculator"


def test_response_content():
    assertion = AgentAssertion.response_content()
    assert isinstance(assertion, AgentAssertion.ResponseContent)


def test_response_model():
    assertion = AgentAssertion.response_model()
    assert isinstance(assertion, AgentAssertion.ResponseModel)


def test_response_finish_reason():
    assertion = AgentAssertion.response_finish_reason()
    assert isinstance(assertion, AgentAssertion.ResponseFinishReason)


def test_response_input_tokens():
    assertion = AgentAssertion.response_input_tokens()
    assert isinstance(assertion, AgentAssertion.ResponseInputTokens)


def test_response_output_tokens():
    assertion = AgentAssertion.response_output_tokens()
    assert isinstance(assertion, AgentAssertion.ResponseOutputTokens)


def test_response_total_tokens():
    assertion = AgentAssertion.response_total_tokens()
    assert isinstance(assertion, AgentAssertion.ResponseTotalTokens)


def test_response_field():
    assertion = AgentAssertion.response_field("candidates[0].safety_ratings[0].category")
    assert isinstance(assertion, AgentAssertion.ResponseField)
    assert assertion.path == "candidates[0].safety_ratings[0].category"


# ─── AgentAssertionTask construction tests ────────────────────────────────


def test_task_creation():
    task = AgentAssertionTask(
        id="check_tool",
        assertion=AgentAssertion.tool_called("web_search"),
        expected_value=True,
        operator=ComparisonOperator.Equals,
    )
    assert task.id == "check_tool"
    assert isinstance(task.assertion, AgentAssertion.ToolCalled)
    assert task.operator == ComparisonOperator.Equals
    assert task.expected_value is True


def test_task_id_lowercase():
    task = AgentAssertionTask(
        id="CheckTool",
        assertion=AgentAssertion.tool_called("web_search"),
        expected_value=True,
        operator=ComparisonOperator.Equals,
    )
    assert task.id == "checktool"


def test_task_with_description():
    task = AgentAssertionTask(
        id="check_tool",
        assertion=AgentAssertion.tool_called("web_search"),
        expected_value=True,
        operator=ComparisonOperator.Equals,
        description="Verify web_search was called",
    )
    assert task.description == "Verify web_search was called"


def test_task_with_dependencies():
    task = AgentAssertionTask(
        id="check_tokens",
        assertion=AgentAssertion.response_output_tokens(),
        expected_value=1000,
        operator=ComparisonOperator.LessThan,
        depends_on=["check_tool"],
    )
    assert len(task.depends_on) == 1
    assert "check_tool" in task.depends_on


def test_task_with_condition():
    task = AgentAssertionTask(
        id="gate_task",
        assertion=AgentAssertion.tool_called("web_search"),
        expected_value=True,
        operator=ComparisonOperator.Equals,
        condition=True,
    )
    assert task.condition is True


def test_task_string_representation():
    task = AgentAssertionTask(
        id="check_tool",
        assertion=AgentAssertion.tool_called("web_search"),
        expected_value=True,
        operator=ComparisonOperator.Equals,
    )
    task_str = str(task)
    assert "check_tool" in task_str
    assert isinstance(task_str, str)


# ─── execute_agent_assertion_tasks tests (tool call assertions) ───────────


def test_execute_tool_called_openai():
    """Test tool_called assertion with OpenAI format."""
    context = {
        "model": "gpt-4o",
        "choices": [
            {
                "message": {
                    "role": "assistant",
                    "content": None,
                    "tool_calls": [
                        {
                            "id": "call_123",
                            "type": "function",
                            "function": {
                                "name": "web_search",
                                "arguments": '{"query": "weather NYC"}',
                            },
                        }
                    ],
                },
                "finish_reason": "tool_calls",
            }
        ],
        "usage": {"prompt_tokens": 10, "completion_tokens": 50, "total_tokens": 60},
    }

    results = execute_agent_assertion_tasks(
        tasks=[
            AgentAssertionTask(
                id="search_called",
                assertion=AgentAssertion.tool_called("web_search"),
                expected_value=True,
                operator=ComparisonOperator.Equals,
            ),
            AgentAssertionTask(
                id="no_delete",
                assertion=AgentAssertion.tool_not_called("delete_user"),
                expected_value=True,
                operator=ComparisonOperator.Equals,
            ),
            AgentAssertionTask(
                id="tool_count",
                assertion=AgentAssertion.tool_call_count(),
                expected_value=1,
                operator=ComparisonOperator.Equals,
            ),
        ],
        context={"response": context},
    )

    assert results["search_called"].passed
    assert results["no_delete"].passed
    assert results["tool_count"].passed


def test_execute_tool_called_with_args():
    """Test partial argument matching."""
    context = {
        "model": "gpt-4o",
        "choices": [
            {
                "message": {
                    "role": "assistant",
                    "content": None,
                    "tool_calls": [
                        {
                            "id": "call_1",
                            "type": "function",
                            "function": {
                                "name": "web_search",
                                "arguments": '{"query": "weather NYC", "lang": "en", "limit": 5}',
                            },
                        }
                    ],
                },
                "finish_reason": "tool_calls",
            }
        ],
        "usage": {"prompt_tokens": 10, "completion_tokens": 50, "total_tokens": 60},
    }

    results = execute_agent_assertion_tasks(
        tasks=[
            AgentAssertionTask(
                id="partial_match",
                assertion=AgentAssertion.tool_called_with_args("web_search", {"query": "weather NYC"}),
                expected_value=True,
                operator=ComparisonOperator.Equals,
            ),
            AgentAssertionTask(
                id="wrong_args",
                assertion=AgentAssertion.tool_called_with_args("web_search", {"query": "weather LA"}),
                expected_value=False,
                operator=ComparisonOperator.Equals,
            ),
        ],
        context={"response": context},
    )

    assert results["partial_match"].passed
    assert results["wrong_args"].passed


def test_execute_tool_call_sequence():
    """Test tool call sequence matching."""
    context = {
        "model": "gpt-4o",
        "choices": [
            {
                "message": {
                    "role": "assistant",
                    "content": None,
                    "tool_calls": [
                        {"id": "call_1", "type": "function", "function": {"name": "web_search", "arguments": "{}"}},
                        {"id": "call_2", "type": "function", "function": {"name": "summarize", "arguments": "{}"}},
                        {"id": "call_3", "type": "function", "function": {"name": "respond", "arguments": "{}"}},
                    ],
                },
                "finish_reason": "tool_calls",
            }
        ],
        "usage": {"prompt_tokens": 10, "completion_tokens": 50, "total_tokens": 60},
    }

    results = execute_agent_assertion_tasks(
        tasks=[
            AgentAssertionTask(
                id="correct_order",
                assertion=AgentAssertion.tool_call_sequence(["web_search", "summarize", "respond"]),
                expected_value=True,
                operator=ComparisonOperator.Equals,
            ),
            AgentAssertionTask(
                id="wrong_order",
                assertion=AgentAssertion.tool_call_sequence(["respond", "web_search"]),
                expected_value=False,
                operator=ComparisonOperator.Equals,
            ),
        ],
        context=context,
    )

    assert results["correct_order"].passed
    assert results["wrong_order"].passed


def test_execute_tool_argument_extraction():
    """Test extracting tool argument values."""

    context = {
        "model": "gpt-4o",
        "choices": [
            {
                "message": {
                    "role": "assistant",
                    "content": None,
                    "tool_calls": [
                        {
                            "id": "call_1",
                            "type": "function",
                            "function": {
                                "name": "web_search",
                                "arguments": '{"query": "test query", "limit": 10}',
                            },
                        }
                    ],
                },
                "finish_reason": "tool_calls",
            }
        ],
        "usage": {"prompt_tokens": 10, "completion_tokens": 50, "total_tokens": 60},
    }

    results = execute_agent_assertion_tasks(
        tasks=[
            AgentAssertionTask(
                id="check_query",
                assertion=AgentAssertion.tool_argument("web_search", "query"),
                expected_value="test query",
                operator=ComparisonOperator.Equals,
            ),
            AgentAssertionTask(
                id="check_limit",
                assertion=AgentAssertion.tool_argument("web_search", "limit"),
                expected_value=10,
                operator=ComparisonOperator.Equals,
            ),
        ],
        context=context,
    )

    assert results["check_query"].passed
    assert results["check_limit"].passed


def test_execute_response_content():
    """Test response content assertion."""
    context = {
        "model": "gpt-4o",
        "choices": [
            {
                "message": {
                    "role": "assistant",
                    "content": "The sky is blue due to Rayleigh scattering.",
                },
                "finish_reason": "stop",
            }
        ],
    }

    results = execute_agent_assertion_tasks(
        tasks=[
            AgentAssertionTask(
                id="check_content",
                assertion=AgentAssertion.response_content(),
                expected_value="Rayleigh scattering",
                operator=ComparisonOperator.Contains,
            ),
        ],
        context=context,
    )

    assert results["check_content"].passed


def test_execute_response_model():
    """Test response model assertion."""
    context = {
        "model": "gpt-4o",
        "choices": [
            {
                "message": {"role": "assistant", "content": "Hello!"},
                "finish_reason": "stop",
            }
        ],
    }

    results = execute_agent_assertion_tasks(
        tasks=[
            AgentAssertionTask(
                id="check_model",
                assertion=AgentAssertion.response_model(),
                expected_value="gpt-4o",
                operator=ComparisonOperator.Equals,
            ),
        ],
        context=context,
    )

    assert results["check_model"].passed


def test_execute_response_tokens():
    """Test token count assertions."""
    context = {
        "model": "gpt-4o",
        "choices": [
            {
                "message": {"role": "assistant", "content": "Hello!"},
                "finish_reason": "stop",
            }
        ],
        "usage": {"prompt_tokens": 10, "completion_tokens": 50, "total_tokens": 60},
    }

    results = execute_agent_assertion_tasks(
        tasks=[
            AgentAssertionTask(
                id="input_tokens",
                assertion=AgentAssertion.response_input_tokens(),
                expected_value=100,
                operator=ComparisonOperator.LessThan,
            ),
            AgentAssertionTask(
                id="output_tokens",
                assertion=AgentAssertion.response_output_tokens(),
                expected_value=1000,
                operator=ComparisonOperator.LessThan,
            ),
            AgentAssertionTask(
                id="total_tokens",
                assertion=AgentAssertion.response_total_tokens(),
                expected_value=60,
                operator=ComparisonOperator.Equals,
            ),
        ],
        context=context,
    )

    assert results["input_tokens"].passed
    assert results["output_tokens"].passed
    assert results["total_tokens"].passed


# ─── Vendor auto-detection tests ────────────────────────────────────────────


def test_anthropic_format():
    """Test assertions against Anthropic response format."""
    context = {
        "id": "msg_123",
        "type": "message",
        "role": "assistant",
        "model": "claude-sonnet-4-20250514",
        "content": [
            {"type": "text", "text": "Let me search for that."},
            {
                "type": "tool_use",
                "id": "toolu_123",
                "name": "web_search",
                "input": {"query": "weather NYC"},
            },
        ],
        "stop_reason": "tool_use",
        "usage": {"input_tokens": 100, "output_tokens": 200},
    }

    results = execute_agent_assertion_tasks(
        tasks=[
            AgentAssertionTask(
                id="check_model",
                assertion=AgentAssertion.response_model(),
                expected_value="claude-sonnet-4-20250514",
                operator=ComparisonOperator.Equals,
            ),
            AgentAssertionTask(
                id="check_content",
                assertion=AgentAssertion.response_content(),
                expected_value="search",
                operator=ComparisonOperator.Contains,
            ),
            AgentAssertionTask(
                id="check_tool",
                assertion=AgentAssertion.tool_called("web_search"),
                expected_value=True,
                operator=ComparisonOperator.Equals,
            ),
            AgentAssertionTask(
                id="check_finish",
                assertion=AgentAssertion.response_finish_reason(),
                expected_value="tool_use",
                operator=ComparisonOperator.Equals,
            ),
        ],
        context=context,
    )

    assert results["check_model"].passed
    assert results["check_content"].passed
    assert results["check_tool"].passed
    assert results["check_finish"].passed


def test_google_format():
    """Test assertions against Google/Gemini response format."""
    context = {
        "candidates": [
            {
                "content": {
                    "role": "model",
                    "parts": [
                        {"text": "The sky appears blue due to Rayleigh scattering."},
                        {
                            "functionCall": {
                                "name": "web_search",
                                "args": {"query": "sky color science"},
                            }
                        },
                    ],
                },
                "finishReason": "STOP",
            }
        ],
        "usageMetadata": {
            "promptTokenCount": 10,
            "candidatesTokenCount": 50,
        },
    }

    results = execute_agent_assertion_tasks(
        tasks=[
            AgentAssertionTask(
                id="check_content",
                assertion=AgentAssertion.response_content(),
                expected_value="Rayleigh scattering",
                operator=ComparisonOperator.Contains,
            ),
            AgentAssertionTask(
                id="check_tool",
                assertion=AgentAssertion.tool_called("web_search"),
                expected_value=True,
                operator=ComparisonOperator.Equals,
            ),
            AgentAssertionTask(
                id="check_finish",
                assertion=AgentAssertion.response_finish_reason(),
                expected_value="STOP",
                operator=ComparisonOperator.Equals,
            ),
            AgentAssertionTask(
                id="check_input_tokens",
                assertion=AgentAssertion.response_input_tokens(),
                expected_value=10,
                operator=ComparisonOperator.Equals,
            ),
        ],
        context=context,
    )

    assert results["check_content"].passed
    assert results["check_tool"].passed
    assert results["check_finish"].passed
    assert results["check_input_tokens"].passed


def test_response_sub_key():
    """Test extracting from response sub-key wrapping."""
    context = {
        "request": {"messages": [{"role": "user", "content": "hi"}]},
        "response": {
            "model": "gpt-4o",
            "choices": [
                {
                    "message": {"role": "assistant", "content": "Hello!"},
                    "finish_reason": "stop",
                }
            ],
            "usage": {
                "prompt_tokens": 5,
                "completion_tokens": 3,
                "total_tokens": 8,
            },
        },
    }

    results = execute_agent_assertion_tasks(
        tasks=[
            AgentAssertionTask(
                id="check_model",
                assertion=AgentAssertion.response_model(),
                expected_value="gpt-4o",
                operator=ComparisonOperator.Equals,
            ),
            AgentAssertionTask(
                id="check_content",
                assertion=AgentAssertion.response_content(),
                expected_value="Hello!",
                operator=ComparisonOperator.Equals,
            ),
        ],
        context=context,
    )

    assert results["check_model"].passed
    assert results["check_content"].passed


def test_response_field_escape_hatch():
    """Test ResponseField for vendor-specific field extraction.

    The path is resolved against the parsed response value (not the full context dict),
    so it must be relative to the response object itself.
    """
    context = {
        "response": {
            "candidates": [
                {
                    "content": {"role": "model", "parts": [{"text": "hello"}]},
                    "finishReason": "STOP",
                    "safety_ratings": [{"category": "HARM_CATEGORY_SAFE"}],
                }
            ],
            "usageMetadata": {"promptTokenCount": 5, "candidatesTokenCount": 2},
        }
    }

    results = execute_agent_assertion_tasks(
        tasks=[
            AgentAssertionTask(
                id="check_safety",
                assertion=AgentAssertion.response_field("candidates[0].safety_ratings[0].category"),
                expected_value="HARM_CATEGORY_SAFE",
                operator=ComparisonOperator.Equals,
            ),
        ],
        context=context,
    )

    assert results["check_safety"].passed


# ─── EvalDataset integration test ───────────────────────────────────────────


def test_eval_dataset_with_agent_assertions():
    """End-to-end: EvalDataset with request assertion tasks."""
    tasks = [
        AgentAssertionTask(
            id="search_called",
            assertion=AgentAssertion.tool_called("web_search"),
            expected_value=True,
            operator=ComparisonOperator.Equals,
        ),
        AgentAssertionTask(
            id="check_content",
            assertion=AgentAssertion.response_content(),
            expected_value="Rayleigh",
            operator=ComparisonOperator.Contains,
        ),
        AgentAssertionTask(
            id="check_model",
            assertion=AgentAssertion.response_model(),
            expected_value="gpt-4o",
            operator=ComparisonOperator.Equals,
        ),
        AgentAssertionTask(
            id="no_delete",
            assertion=AgentAssertion.tool_not_called("delete_user"),
            expected_value=True,
            operator=ComparisonOperator.Equals,
        ),
        AgentAssertionTask(
            id="check_tokens",
            assertion=AgentAssertion.response_output_tokens(),
            expected_value=1000,
            operator=ComparisonOperator.LessThan,
        ),
    ]

    record = EvalRecord(
        context={
            "response": {
                "model": "gpt-4o",
                "choices": [
                    {
                        "message": {
                            "role": "assistant",
                            "content": "The sky is blue due to Rayleigh scattering.",
                            "tool_calls": [
                                {
                                    "id": "call_1",
                                    "type": "function",
                                    "function": {
                                        "name": "web_search",
                                        "arguments": '{"query": "sky color"}',
                                    },
                                }
                            ],
                        },
                        "finish_reason": "stop",
                    }
                ],
                "usage": {
                    "prompt_tokens": 10,
                    "completion_tokens": 50,
                    "total_tokens": 60,
                },
            }
        },
        id="openai_test",
    )

    dataset = EvalDataset(records=[record], tasks=tasks)
    results = dataset.evaluate()
    result = results["openai_test"]

    assert result.eval_set.passed_tasks == 5
    assert result.eval_set.failed_tasks == 0
    assert result.eval_set.pass_rate == 1.0


def test_eval_dataset_google_adk_format():
    """End-to-end: EvalDataset with Google ADK response format."""
    tasks = [
        AgentAssertionTask(
            id="check_content",
            assertion=AgentAssertion.response_content(),
            expected_value="Rayleigh scattering",
            operator=ComparisonOperator.Contains,
        ),
        AgentAssertionTask(
            id="search_called",
            assertion=AgentAssertion.tool_called("web_search"),
            expected_value=True,
            operator=ComparisonOperator.Equals,
        ),
        AgentAssertionTask(
            id="check_args",
            assertion=AgentAssertion.tool_called_with_args("web_search", {"query": "sky color science"}),
            expected_value=True,
            operator=ComparisonOperator.Equals,
        ),
    ]

    record = EvalRecord(
        context={
            "request": {
                "contents": [
                    {
                        "role": "user",
                        "parts": [{"text": "Why is the sky blue?"}],
                    }
                ]
            },
            "response": {
                "candidates": [
                    {
                        "content": {
                            "role": "model",
                            "parts": [
                                {"text": "The sky appears blue due to Rayleigh scattering."},
                                {
                                    "functionCall": {
                                        "name": "web_search",
                                        "args": {"query": "sky color science"},
                                    }
                                },
                            ],
                        },
                        "finishReason": "STOP",
                    }
                ],
                "usageMetadata": {
                    "promptTokenCount": 10,
                    "candidatesTokenCount": 50,
                },
            },
        },
        id="google_test",
    )

    dataset = EvalDataset(records=[record], tasks=tasks)
    results = dataset.evaluate()
    result = results["google_test"]

    assert result.eval_set.passed_tasks == 3
    assert result.eval_set.failed_tasks == 0


def test_eval_dataset_mixed_task_types():
    """Test EvalDataset with both assertion and request assertion tasks."""
    from scouter.evaluate import AssertionTask

    tasks = [
        # Regular assertion on context field
        AssertionTask(
            id="check_input",
            context_path="request.contents[0].role",
            expected_value="user",
            operator=ComparisonOperator.Equals,
        ),
        # Request assertion on normalized response
        AgentAssertionTask(
            id="check_tool",
            assertion=AgentAssertion.tool_called("web_search"),
            expected_value=True,
            operator=ComparisonOperator.Equals,
        ),
    ]

    record = EvalRecord(
        context={
            "request": {
                "contents": [
                    {"role": "user", "parts": [{"text": "test"}]},
                ]
            },
            "response": {
                "candidates": [
                    {
                        "content": {
                            "role": "model",
                            "parts": [
                                {
                                    "functionCall": {
                                        "name": "web_search",
                                        "args": {"query": "test"},
                                    }
                                }
                            ],
                        },
                        "finishReason": "STOP",
                    }
                ]
            },
        },
        id="mixed_test",
    )

    dataset = EvalDataset(records=[record], tasks=tasks)
    results = dataset.evaluate()
    result = results["mixed_test"]

    assert result.eval_set.passed_tasks == 2
    assert result.eval_set.failed_tasks == 0
