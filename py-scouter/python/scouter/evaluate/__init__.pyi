# type: ignore
# pylint: disable=redefined-builtin
from typing import Any, Dict, Iterator, List, Optional, Protocol, TypeAlias, Union

from ..llm import Prompt, Score

class BaseModel(Protocol):
    """Protocol for pydantic BaseModel to ensure compatibility with context"""

    def model_dump(self) -> Dict[str, Any]:
        """Dump the model as a dictionary"""

    def model_dump_json(self) -> str:
        """Dump the model as a JSON string"""

    def __str__(self) -> str:
        """String representation of the model"""

SerializedType: TypeAlias = Union[str, int, float, dict, list]
Context: TypeAlias = Union[Dict[str, Any], BaseModel]

class EvalResult:
    """Eval Result for a specific evaluation"""

    @property
    def error(self) -> Optional[str]: ...
    @property
    def tasks(self) -> Dict[str, Score]: ...
    @property
    def id(self) -> str: ...
    def __getitem__(self, key: str) -> Score:
        """
        Get the score for a specific task
        """

    def to_dataframe(self, polars: bool = False) -> Any:
        """Converts the evaluation results to a pandas DataFrame

        Args:
            polars (bool):
                Whether to convert to a Polars DataFrame.
        """

class LLMEvalResults:
    """Defines the results of an LLM eval metric"""

    def __getitem__(self, key: str) -> float:
        """Get the value` of the metric by name. A RuntimeError will be raised if the metric does not exist."""

    def __iter__(self) -> Iterator[EvalResult]:
        """Get an iterator over the metric names and values."""

class LLMEvalMetric:
    """Defines an LLM eval metric to use when evaluating LLMs"""

    def __init__(self, name: str, prompt: Prompt):
        """
        Initialize an LLMEvalMetric to use for evaluating LLMs. This is
        most commonly used in conjunction with `evaluate_llm` where LLM inputs
        and responses can be evaluated against a variety of user-defined metrics.

        Args:
            name (str):
                Name of the metric
            prompt (Prompt):
                Prompt to use for the metric. For example, a user may create
                an accuracy analysis prompt or a query reformulation analysis prompt.
        """

    def __str__(self) -> str:
        """
        String representation of the LLMEvalMetric
        """

class LLMEvalRecord:
    """LLM record containing context tied to a Large Language Model interaction
    that is used to evaluate LLM responses.


    Examples:
        >>> record = LLMEvalRecord(
                id="123",
                context={
                    "input": "What is the capital of France?",
                    "response": "Paris is the capital of France."
                },
        ... )
        >>> print(record.context["input"])
        "What is the capital of France?"
    """

    def __init__(
        self,
        context: Context,
        id: Optional[str] = None,
    ) -> None:
        """Creates a new LLM record to associate with an `LLMDriftProfile`.
        The record is sent to the `Scouter` server via the `ScouterQueue` and is
        then used to inject context into the evaluation prompts.

        Args:
            context:
                Additional context information as a dictionary or a pydantic BaseModel. During evaluation,
                this will be merged with the input and response data and passed to the assigned
                evaluation prompts. So if you're evaluation prompts expect additional context via
                bound variables (e.g., `${foo}`), you can pass that here as key value pairs.
                {"foo": "bar"}
            id:
                Unique identifier for the record. If not provided, a new UUID will be generated.
                This is helpful for when joining evaluation results back to the original request.

        Raises:
            TypeError: If context is not a dict or a pydantic BaseModel.

        """

    @property
    def context(self) -> Dict[str, Any]:
        """Get the contextual information.

        Returns:
            The context data as a Python object (deserialized from JSON).
        """

def evaluate_llm(
    records: List[LLMEvalRecord],
    metrics: List[LLMEvalMetric],
) -> LLMEvalResults:
    """
    Evaluate LLM responses using the provided evaluation metrics.

    Args:
        records (List[LLMEvalRecord]):
            List of LLM evaluation records to evaluate.
        metrics (List[LLMEvalMetric]):
            List of LLMEvalMetric instances to use for evaluation.

    Returns:
        LLMEvalResults
    """
