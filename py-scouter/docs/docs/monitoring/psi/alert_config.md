from scouter.types import CommonCrons

# PSI Alert Configuration

Configure how you and your team want to be alerted in the event of model drift using `PsiAlertConfig`.

```py
from scouter.alert import PsiAlertConfig, SlackDispatchConfig
from scouter.types import CommonCrons

PsiAlertConfig(
    dispatch_config=SlackDispatchConfig(channel='my-team-channel'),
    schedule=CommonCrons.Every6Hours,
    features_to_monitor=['feature_1', 'feature_2', ....],
)
```

## Parameters

| Parameter       | Type                 | Description                                                                                                          | Example |
|-----------------|----------------------|----------------------------------------------------------------------------------------------------------------------|---------|
| dispatch_config | `SlackDispatchConfig | OpsGenieDispatchConfig | None`            | An optional dispatch configuration used to configure how alerts are routed, if None is provided a default internal dispatch type of Console will be used to log alerts to the conosole of the scouter server.  | `config.dispatch_config -> SlackDispatchConfig()` |
| schedule        | `str | CommonCrons | None`                | Schedule to run drift detection job. Defaults to daily at midnigh. You can use the builtin CommonCron options or specify your own custom cron.                  | `config.schedule → CommonCrons.Every6Hours` |
| features_to_monitor         | `str`                | The version of the model or dataset being monitored. Defaults to '0.0.1' if not provided.                            | `config.version → "0.0.1"` |
| alert_config    | `PsiAlertConfig`     | Configuration for alerting when drift is detected. Defaults to the default implementation of PsiAlertConfig if not provided. | `config.alert_config → *Instance of PsiAlertConfig*` |

## Properties


| Property       | Type             | Description                                                                                                                                        | Example                                            |
|----------------|------------------|----------------------------------------------------------------------------------------------------------------------------------------------------|----------------------------------------------------|
| `name`         | `str`            | The name of the model or dataset being monitored.                                                                                                  | `config.name` → `"wine_model"`                     |
| `repository`   | `str`            | The repository where the model or dataset is stored.                                                                                               | `config.repository` → `"wine_model"`               |
| `version`      | `str`            | The version of the model or dataset being monitored.                                                                                               | `config.version` → `"0.0.1"`                       |
| `feature_map`  | `FeatureMap`     | When a non-numeric covariate is detected, each unique value is assigned a corresponding numeric value. This mapping is represented by feature_map. | `config.feature_map` → `Instance of FeatureMap`      |
| `targets`      | `list[str]`      | List of target features, typically the dependent variable(s).                                                                                      | `config.targets` → `label`                         |
| `alert_config` | `PsiAlertConfig` | Configuration for alerting when drift is detected.                                                                                                 | `config.alert_config` → `Instance of PsiAlertConfig` |
| `drift_type`   | `DriftType`      | Type of drift profile.                                                                                                                             | `config.drift_type` → `DriftType.Psi`              |



## Methods

### `load_from_json_file()`
Loads a `PsiDriftConfig` instance from a JSON file.

- **Parameters:**
    - **`path`** (`Path`): The path to the JSON configuration file. This is required to locate and read the configuration file from disk.
- **Returns:** A `PsiDriftConfig` instance.
- **Return Type:** `PsiDriftConfig`

---

### `model_dump_json()`
Serializes the `PsiDriftConfig` instance to a JSON string.

- **Parameters:** None
- **Returns:** A JSON string representation of the instance.
- **Return Type:** `str`

---

### `update_config_args()`
Updates the configuration of the instance with new values.

- **Parameters:**
    - **`repository`** (`Optional[str]`): The repository name, if updating.
    - **`name`** (`Optional[str]`): The new name for the configuration, if provided.
    - **`version`** (`Optional[str]`): The version to set, if specified.
    - **`targets`** (`Optional[List[str]]`): A list of target identifiers, if updating.
    - **`alert_config`** (`Optional[PsiAlertConfig]`): The alert configuration, if provided.
- **Returns:** `None`
- **Return Type:** `None`
