from __future__ import annotations

from collections.abc import AsyncGenerator

from google.adk.models.base_llm import BaseLlm
from google.adk.models.llm_request import LlmRequest
from google.adk.models.llm_response import LlmResponse
from google.genai import types

MODEL_NAME = "gemini-2.0-flash"


class DeterministicToolCallingLlm(BaseLlm):
    def __init__(
        self,
        *,
        tool_name: str,
        tool_args: dict[str, str],
        final_text: str,
    ) -> None:
        super().__init__(model=MODEL_NAME)
        object.__setattr__(self, "_tool_name", tool_name)
        object.__setattr__(self, "_tool_args", tool_args)
        object.__setattr__(self, "_final_text", final_text)
        object.__setattr__(self, "_calls", 0)

    async def generate_content_async(
        self,
        llm_request: LlmRequest,
        stream: bool = False,
    ) -> AsyncGenerator[LlmResponse, None]:
        assert stream is False

        calls = int(getattr(self, "_calls"))
        object.__setattr__(self, "_calls", calls + 1)

        if calls == 0:
            yield LlmResponse(
                model_version=llm_request.model,
                content=types.Content(
                    role="model",
                    parts=[
                        types.Part.from_function_call(
                            name=str(getattr(self, "_tool_name")),
                            args=dict(getattr(self, "_tool_args")),
                        )
                    ],
                ),
                partial=False,
                finish_reason=types.FinishReason.STOP,
            )
            return

        yield LlmResponse(
            model_version=llm_request.model,
            content=types.Content(
                role="model",
                parts=[types.Part(text=str(getattr(self, "_final_text")))],
            ),
            partial=False,
            finish_reason=types.FinishReason.STOP,
            usage_metadata=types.GenerateContentResponseUsageMetadata(
                prompt_token_count=21,
                candidates_token_count=13,
                total_token_count=34,
            ),
        )
