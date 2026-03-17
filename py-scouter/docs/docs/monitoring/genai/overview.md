## GenAI Evaluation and Drift Detection Overview

Similar to PSI, SPC and Custom Drift Profiles, Scouter provides support for both offline and on-line GenAI evaluations/drift detection of GenAI services.


## Why monitor GenAI Services?

This point has been made multiple times by others, so we won't rehash it here, but evaluating GenAI services is critical for ensuring the quality and reliability of your LLM-powered applications. GenAI applications may hallucinate or output content that's not quite in line with what you expect. And for customer-facing applications, this can lead to poor user experiences or even harmful outcomes. So knowing when you service is drifting from expected behavior is crucial. Monitoring/Evaluating GenAI services is often important in offline settings as well, where you may want to run batch evaluations for regression testing or model comparisons.


## What does Scouter provide for GenAI Evaluation?

### Building Blocks for GenAI Evaluations

Before going over offline and online evaluations, it's important to understand how tasks work in Scouter for GenAI evaluations. Scouter provides four task types: `AssertionTask` (deterministic rule-based checks), `LLMJudgeTask` (LLM-powered semantic evaluation), `TraceAssertionTask` (trace/span property validation), and `AgentAssertionTask` (deterministic assertions on agent tool calls and response properties — auto-detects OpenAI, Anthropic, and Google response formats). Tasks allow you to define expected outputs and evaluation criteria for your GenAI services and can be chained together to create complex evaluation workflows. More on this can be found in the [Task Building Blocks Section](/scouter/docs/monitoring/genai/tasks/).

### Offline Evaluation

One of our goals with GenAI evaluations is to maintain parity between offline and online evaluations. This means you can define your evaluation tasks once and use them both for offline batch evaluations as well as on-line. This ensures consistency in how you measure your LLM's performance across different environments and versions. To run offline evaluations, you can use the `EvalDataset` along with the `EvalRecord` class and `LLMJudgeTask` and `AssertionTask` to run evaluations against a set of records. More on this can be found in the [Offline Evaluation Documentation](/scouter/docs/monitoring/genai/offline-evaluation/).


### Online Drift Detection

In line with our other drift tooling, Scouter provides a way to define GenAI Eval Profiles that can be used to monitor your LLM services in real-time. These profiles allow you to specify both tasks and alert criteria, so you can be notified when your LLM's performance degrades or drifts from expected behavior. This is done using the `GenAIEvalProfile`, `GenAIEvalConfig`, `LLMJudgeTask`, `AssertionTask`, and `AgentAssertionTask` classes. More on this can be found in the [Online Evaluation Documentation](/scouter/docs/monitoring/genai/online-evaluation/).