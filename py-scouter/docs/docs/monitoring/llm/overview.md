## Creating LLM Drift Profiles

LLM Drift Profiles in Scouter provide a robust and flexible way to monitor the performance and stability of Large Language Models (LLMs) over time. By defining custom metrics, prompts, and workflows, you can detect drift and set up alerting tailored to your use case.

## What is an LLM Drift Profile?

An **LLM Drift Profile** encapsulates:

- The configuration for drift monitoring (LLMDriftConfig)
- The metrics to evaluate (LLMMetric)
- Optionally, a custom workflow (Workflow) for advanced scenarios

This profile can be used to monitor LLMs for changes in output quality, relevance, or any other custom metric you define.

## Steps to Create an LLM Drift Profile

### 1. Define LLM Metrics

The `LLMMetric` class represents a single metric for LLM drift detection.

**Arguments:**

| Argument                | Type              | Required | Description                                                        |
|-------------------------|-------------------|----------|--------------------------------------------------------------------|
| `name`                  | `str`             | Yes      | Name of the metric (e.g., "accuracy", "relevance")                 |
| `value`                 | `float`           | Yes      | Baseline value for the metric                                      |
| `alert_threshold`       | `AlertThreshold`  | Yes      | Condition for triggering an alert (e.g., `Above`, `Below`)         |
| `alert_threshold_value` | `float`, optional | No       | Threshold value for alerting                                       |
| `prompt`                | `Prompt`, optional| No       | Prompt associated with the metric (required if not using workflow) |

**Example:**
```python
from scouter.llm import AlertThreshold, Prompt, Score
from scouter.drift import LLMMetric

reformulation_prompt = (
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
)

metric = LLMMetric(
    name="accuracy",
    value=0.95,
    alert_threshold=AlertThreshold.Below,
    alert_threshold_value=0.9,
    prompt=Prompt(
        user_message=reformulation_prompt,
        model="gpt-4o",
        provider="openai",
        response_format=Score
    )
)
```

#### Prompt Requirements

When defining prompts for LLM metrics, ensure they include the following:

- **Input Parameters:**  
    - Each prompt must include at least one named parameter. An error will be raised if the prompt does not include a parameter
    (e.g., `${input}`, `${response}`, `${user_query}`) to allow Scouter to inject the relevant data during evaluation.
    - Named parameters must follow the `${parameter_name}` format.

- **Score Response Format:**
    All evaluation prompts must use the `Score` response format. The prompt should instruct the model to return a JSON object matching the `Score` schema:
      - `score`: An integer value (typically 1–5) representing the evaluation result.
      - `reason`: A brief explanation for the score.

### 2. Create an LLM Drift Config

The `LLMDriftConfig` class defines the configuration for drift monitoring.

**Arguments:**

| Argument       | Type             | Required | Description                                   |
|----------------|------------------|----------|-----------------------------------------------|
| `space`        | `str`            | No       | Model space (default: `"__missing__"`)        |
| `name`         | `str`            | No       | Model name (default: `"__missing__"`)         |
| `version`      | `str`            | No       | Model version (default: `"0.1.0"`)            |
| `sample_rate`  | `int`            | No       | Sample rate for drift detection (default: 5 (1 out of 5))  |
| `alert_config` | `LLMAlertConfig` | No       | Alert configuration                           |

**Example:**
```python
from scouter.llm import LLMDriftConfig

config = LLMDriftConfig(
    space="my_space",
    name="my_model",
    version="1.0.0",
    sample_rate=10
)
```

### 3. (Optional) Define a Custom Workflow

For advanced scenarios, you can provide a custom `Workflow` to evaluate complex pipelines or multi-step tasks.

- All metric names must match the final task names in the workflow.
- Final tasks must use the `Score` response type.

**Example:**
```python
from scouter.llm import Workflow, Task, Score, Agent, Prompt
from scouter.drift import LLMDriftConfig, LLMMetric

# Relevance prompt
relevance_prompt = Prompt(
    user_message=(
        "Given the following input and response, rate the relevance of the response to the input on a scale of 1 to 5.\n\n"
        "Input: ${input}\n" # (1)
        "Response: ${response}\n\n"
        "Provide a brief reason for your rating."
    ),
    system_message="You are a helpful assistant that evaluates relevance.",
    response_format=Score
)

# Coherence prompt
coherence_prompt = Prompt(
    user_message=(
        "Given the following response, rate its coherence and logical consistency on a scale of 1 to 5.\n\n"
        "Response: ${response}\n\n"
        "Provide a brief reason for your rating."
    ),
    system_message="You are a helpful assistant that evaluates coherence.",
    response_format=Score
)

final_eval_prompt = Prompt(
    user_message=(
        "Given the previous relevance and coherence scores for a model response, "
        "determine if the response should PASS or FAIL quality control.\n\n"
        "If both scores are 4 or higher, return a score of 1 and reason 'Pass'. "
        "If either score is below 4, return a score of 0 and reason 'Fail'.\n\n"
        "Respond with a JSON object matching the Score schema."
    ),
    system_message="You are a strict evaluator that only passes high-quality responses.",
    response_format=Score
)

open_agent = Agent("openai")
workflow = Workflow(name="test_workflow")

workflow.add_agent(open_agent)
workflow.add_tasks( # (2)
    [
        Task(
            prompt=relevance_prompt,
            agent_id=open_agent.id,
            id="relevance",
        ),
        Task(
            prompt=coherence_prompt,
            agent_id=open_agent.id,
            id="coherence",
        ),
        Task(
            prompt=final_eval_prompt,
            agent_id=open_agent.id,
            id="final_evaluation",
            depends_on=["relevance", "coherence"],
        ),
    ]
)

metric = LLMMetric( # (3)
    name="final_evaluation",
    value=1,
    alert_threshold=AlertThreshold.Below,
)

profile = LLMDriftProfile(
    config=LLMDriftConfig(),
    workflow=workflow,
    metrics=[metric],
)
```

1. The `${input}` and `${response}` variables will be fed in by Scouter when you record a drift event. Evaluation prompts must include at least one named parameter. `${input}` and `${response}`. It could easily be `${user_query}` or similar, depending on your use case. The important part is that the first tasks in the workflow must include these parameters, so they can be evaluated against the model's output.
2. Here we are creating a directed graph of tasks. The `relevance` and `coherence` tasks are the first tasks, and the `final_evaluation` task depends on them. The final task must return a `Score` type, which is used to extract the metric value on the Scouter server.
3. The metric name must match the final task name in the workflow. The `value` is the baseline score for this metric, and the `alert_threshold` defines when an alert should be triggered based on the metric's value.


### 4. Create the LLM Drift Profile

Use the `LLMDriftProfile` class to create a drift profile by combining your config, metrics, and (optionally) workflow.

**Arguments:**

| Argument    | Type                | Required | Description                                         |
|-------------|---------------------|----------|-----------------------------------------------------|
| `config`    | `LLMDriftConfig`    | Yes      | Drift configuration                                 |
| `metrics`   | `List[LLMMetric]`   | Yes      | List of metrics to monitor                          |
| `workflow`  | `Workflow`, optional| No       | Custom workflow for advanced evaluation (optional)  |


**Example (metrics only):**
```python
from scouter.llm import LLMDriftProfile

profile = LLMDriftProfile(
    config=config,
    metrics=[metric]
)
```

## Prompt Requirements

When creating prompts for LLM Drift Profiles and workflows, the following requirements must be met:

- **Input Parameters:**  
  Each evaluation prompt must include at least one of the standardized parameters: `${input}` or `${response}`.  
  - `${input}`: The original input or context provided to the LLM.
  - `${response}`: The output generated by the LLM.

- **Score Response Format:**  
  All evaluation prompts must use the `Score` response format. The prompt should instruct the model to return a JSON object matching the `Score` schema:
  - `score`: An integer value (typically 1–5) representing the evaluation result.
  - `reason`: A brief explanation for the score.

- **Workflow Tasks:**  
  - The first tasks in a workflow must use prompts that include `${input}` and/or `${response}`.
  - The final tasks in a workflow must return a `Score` object as their response.

**Example Score JSON:**
```json
{
  "score": 5,
  "reason": "The response is highly relevant and coherent."
}
```

## Inserting LLM Drift Data

For a general detailed guide on the `ScouterQueue`, and how to insert data for real-time monitoring in your service, please refer to the [Inference documentation](../inference.md). While the general insertion logic is similar for all drift types, there are a few specific considerations for LLM drift profiles.

### LLM Drift Data Insertion

To insert data for LLM drift profiles, you first create an LLMRecord, which takes the following parameters:

| Argument    | Type                | Required | Description                                         |
|-------------|---------------------|----------|-----------------------------------------------------|
| `input`     | Union[str, int, float, dict, list]  | No   | The input or context provided to the LLM. This is typically the prompt or question you want the model to respond to. |
| `response`  | Union[str, int, float, dict, list]  | No   | The output generated by the LLM. This is the model's response to the input. |
| `context`   | `dict`             | None      | Additional context information as a dictionary. During evaluation, this will be merged with the input and response data and passed to the assigned evaluation prompts. So if you're evaluation prompts expect additional context via bound variables (e.g., `${foo}`), you can pass that here as key value pairs. {"foo": "bar"}. |
| `prompt`     | Prompt or`str`       | Yes      | Optional prompt configuration associated with this record. Can be a `Potatohead` Prompt or a JSON-serializable type.

**Example:**
```python
from scouter.queue import LLMRecord

record = LLMRecord(
    input="What is the capital of France?",
    response="Paris is the capital of France.",
    context={"foo": "bar"}
)

# insert into the ScouterQueue
queue["my_llm_service"].insert(record)
```

### How are the metrics calculated?

When you insert an `LLMRecord`, it contains raw inputs such as `input`, `response`, and `context`, which do not directly match the `LLMMetric` structure defined earlier. So, how does Scouter calculate the metrics?

Scouter is designed to evaluate LLM metrics asynchronously on the server, ensuring your application's performance is not impacted. Here’s how the process works:

1. **Record Ingestion:**  
   When you insert an `LLMRecord`, it is sent to the Scouter server.

2. **Profile Retrieval:**  
   Upon receiving the record, the server retrieves the associated drift profile, which specifies the metrics and workflow to execute.

3. **Prompt Injection & Workflow Execution:**  
   The server injects the `input`, `response`, and `context` from the `LLMRecord` into the prompts defined in the workflow. It then runs the workflow according to your configuration.

4. **Metric Extraction:**  
   After executing the workflow, the server extracts the `Score` object from the relevant tasks as defined by your `LLMMetric`s.

5. **Result Storage & Alerting:**  
   The results are stored in the llm metric table. The system then polls these results based on your alerting schedule. If a score falls below the defined threshold, Scouter triggers an alert and sends it to your configured alerting channel.

This asynchronous evaluation ensures that metric calculation is robust and does not interfere with your service’s real-time performance.

### Architecture Overview

<h1 align="center">
  <br>
  <img src="../../../images/llm-monitoring-arch.png"  width="700"alt="llm monitoring"/>
  <br>
</h1>
## What Scouter Doesn't Do for LLMs

Scouter is meant to be a generic monitoring system for LLM services, meaning we provide you with the basic building blocks to define how **you** want to monitor your services, so that you can integrate it with any framework. In addition, Scouter is not an observability platform, so it does not provide things like LLM tracing. In our experience, tracing != monitoring, and is primarily used for debugging purposes. If you want to trace your LLM services (or any general service), we recommend using a tool like [OpenTelemetry](https://opentelemetry.io/) or similar.

## Examples

Check out the examples directory for more detailed examples of creating and using LLM Drift Profiles in Scouter.