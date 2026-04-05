# Monitor Examples

Online monitoring examples: each is a FastAPI application that registers a drift profile with the Scouter server, then inserts records into a `ScouterQueue` on every inference request. The server samples, aggregates, and checks alert thresholds on a schedule.

## Prerequisites

- A running Scouter server (`make start.server` from the repo root)
- For the GenAI example: a configured LLM provider (see below)

## Examples

### `psi/` — PSI drift monitoring

Monitors feature distributions using Population Stability Index. Detects when production data drifts from a baseline.

**Files:**
- `psi/api.py` — FastAPI app; registers profile on startup, inserts `Features` per request

**Run:**
```bash
cd py-scouter

# 1. Start the API (profile is created and registered on first run)
uv run uvicorn examples.monitor.psi.api:app --reload

# 2. Send a prediction request
curl -X POST http://localhost:8000/predict \
  -H "Content-Type: application/json" \
  -d '{"feature_1": 0.5, "feature_2": -1.2, "feature_3": 0.8}'
```

**What it shows:**

- Creating a `PsiDriftConfig` with `space`, `name`, `version`, and categorical feature binning
- Attaching a `PsiAlertConfig` with a cron schedule (`Every6Hours`)
- Registering the profile with `ScouterClient.register_profile(set_active=True)`
- Loading the profile into `ScouterQueue` via `from_path()`
- Inserting `Features` into the queue on each request — inserts are non-blocking (<1µs)
- Using `FeatureMixin` to convert a Pydantic model to a `Features` object

**Key classes:**

| Class | Purpose |
|-------|---------|
| `Drifter` | Creates drift profiles from baseline data |
| `PsiDriftConfig` | Configures PSI binning strategy and alert schedule |
| `ScouterClient` | Registers profiles with the server |
| `ScouterQueue` | Background queue for inserting records at inference time |
| `Features` / `Feature` | Typed feature container inserted into the queue |

---

### `genai/` — GenAI online evaluation

Monitors a two-stage LLM pipeline (query reformulation → response generation) for quality and relevance in production.

**Files:**
- `genai/api/profile/create_profile.py` — creates and registers the `AgentEvalProfile`
- `genai/api/assets/prompts.py` — Gemini prompt templates and `PromptState`
- `genai/api/main.py` — FastAPI app; runs the LLM pipeline and inserts `EvalRecord`
- `genai/api/assets/genai_drift_profile.json` — saved profile loaded by the app

**Run:**
```bash
cd py-scouter

# Requires Google Gemini credentials
export GOOGLE_API_KEY=<your-key>    # or service account JSON

# 1. Create and register the evaluation profile
uv run python examples/monitor/genai/api/profile/create_profile.py

# 2. Start the API
uv run uvicorn examples.monitor.genai.api.main:app --reload

# 3. Send a question
curl -X POST http://localhost:8000/predict \
  -H "Content-Type: application/json" \
  -d '{"question": "How do I set up a home network?"}'
```

**What it shows:**

- Defining `LLMJudgeTask` tasks that evaluate reformulation quality and response relevance
- Configuring a `AgentEvalProfile` with `sample_ratio` (fraction of traffic to evaluate)
- Setting `AgentAlertConfig` with a `baseline_value`, `AlertThreshold`, and `delta`
- Registering the profile and saving it to JSON for the app to load
- Inserting a `EvalRecord` with a multi-field context dict on each request
- The server sampling records and running evaluation tasks asynchronously

**LLM provider required:** Google Gemini (configured in `prompts.py`). Swap the `Provider` and model settings to use OpenAI or Anthropic instead.

**Key classes:**

| Class | Purpose |
|-------|---------|
| `AgentEvalProfile` | Defines evaluation tasks and alert configuration |
| `AgentEvalConfig` | Profile identity (`space`, `name`, `version`) and `sample_ratio` |
| `LLMJudgeTask` | Semantic evaluation via LLM with structured output |
| `AgentAlertConfig` | Alert threshold and baseline for pass-rate monitoring |
| `EvalRecord` | Captures inference context for evaluation |
| `Agent` / `Prompt` | LLM call abstraction used in the application itself |
