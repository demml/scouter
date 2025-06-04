## PSI Drift Configuration

All models that create a `PsiDriftProfile` will require a `PsiDriftConfig` object.

```py
from scouter.alert import PsiAlertConfig
from scouter.drift import PsiDriftConfig

PsiDriftConfig(
    name="wine_model",
    space="wine_model",
    version="0.0.1",
    alert_config=PsiAlertConfig()
)
```

### Parameters

| Parameter       | Type                  | Description                                                                                                                  | Example                                                    |
|---------------|-----------------------|------------------------------------------------------------------------------------------------------------------------------|------------------------------------------------------------|
| name        | `str`                 | The name of the model or dataset being monitored. Defaults to '\_\_missing\_\_' if not provided.                             | `config.name → "wine_model"`                               |
| space  | `str`                 | The space where the model or dataset is stored. Defaults to '\_\_missing\_\_' if not provided.                               | `config.space → "wine_model"`                              |
| version     | `str`                 | The version of the model or dataset being monitored. Defaults to '0.0.1' if not provided.                                    | `config.version → "0.0.1"`                                 |
| alert_config | `PsiAlertConfig`      | Configuration for alerting when drift is detected. Defaults to the default implementation of PsiAlertConfig if not provided. | `config.alert_config → *Instance of PsiAlertConfig*`       |
| targets     | `list[str]`           | List of target features, typically the dependent variable(s).                                                                | `config.targets → ["churn"]`                               |
| config_path | `Optional[Path]`      | Path to a pre existing PsiDriftConfig. Defaults to None if not provided                                                      | `config.config_path → Path("/configs/drift.yaml")`         |
| categorical_features | `Optional[list[str]]` | To ensure accurate PSI calculations, categorical features must be explicitly specified.                                  | `config.categorical_features → ["feature_1", "feature_2"]` |



### Properties


| Property       | Type             | Description                                                                                                                | Example                                            |
|----------------|------------------|----------------------------------------------------------------------------------------------------------------------------|----------------------------------------------------|
| `name`         | `str`            | The name of the model or dataset being monitored.                                                                          | `config.name` → `"wine_model"`                     |
| `space`   | `str`            | The space where the model or dataset is stored.                                                                            | `config.space` → `"wine_model"`               |
| `version`      | `str`            | The version of the model or dataset being monitored.                                                                       | `config.version` → `"0.0.1"`                       |
| `feature_map`  | `FeatureMap`     | When a non-numeric covariate is detected, each unique value is assigned a corresponding numeric value. This mapping is represented by feature_map. | `config.feature_map` → `Instance of FeatureMap`      |
| `targets`      | `list[str]`      | List of target features, typically the dependent variable(s).                                                              | `config.targets` → `label`                         |
| `alert_config` | `PsiAlertConfig` | Configuration for alerting when drift is detected.                                                                         | `config.alert_config` → `Instance of PsiAlertConfig` |
| `drift_type`   | `DriftType`      | Type of drift profile.                                                                                                     | `config.drift_type` → `DriftType.Psi`              |
| `categorical_features`   | `Optional[list[str]]`      | List of categorical features                                                                                               | `config.categorical_features → ["feature_1", "feature_2"]`              |




### Methods

#### `load_from_json_file()` _(static method)_
Loads a `PsiDriftConfig` instance from a JSON file.

- **Parameters:**
    - **`path`** (`Path`): The path to the JSON configuration file. This is required to locate and read the configuration file from disk.
- **Returns:** A `PsiDriftConfig` instance.
- **Return Type:** `PsiDriftConfig`

---

#### `model_dump_json()`
Serializes the `PsiDriftConfig` instance to a JSON string.

- **Parameters:** None
- **Returns:** A JSON string representation of the instance.
- **Return Type:** `str`

---

#### `update_config_args()`
Updates the configuration of the instance with new values.

- **Parameters:**
    - **`space`** (`Optional[str]`): The space name, if updating.
    - **`name`** (`Optional[str]`): The new name for the configuration, if provided.
    - **`version`** (`Optional[str]`): The version to set, if specified.
    - **`targets`** (`Optional[List[str]]`): A list of target identifiers, if updating.
    - **`alert_config`** (`Optional[PsiAlertConfig]`): The alert configuration, if provided.
- **Returns:** `None`
- **Return Type:** `None`

## Alert Configuration

An `AlertConfig` can also be provided to the `PsiDriftConfig` to specify how you and your team want to be alerted in the event of model drift. The `PsiAlertConfig` class allows you to configure the alerting mechanism, including the dispatch method (e.g., Slack, OpsGenie) and the schedule for drift detection jobs.


```py
from scouter.alert import PsiAlertConfig, SlackDispatchConfig
from scouter.types import CommonCrons

PsiAlertConfig(
    dispatch_config=SlackDispatchConfig(channel='my-team-channel'),
    schedule=CommonCrons.Every6Hours, # (1)
    features_to_monitor=['feature_1', 'feature_2', ...],
)
```

1.  Scouter comes with a set of built-in cron schedules that you can use to configure the schedule for your drift detection job. You can also specify your own custom cron schedule if needed.


### Parameters

| Parameter           | Type                 | Description                                                                   | Example                                                      |
|---------------------|----------------------|-------------------------------------------------------------------------------|--------------------------------------------------------------|
| dispatch_config     | `SlackDispatchConfig | OpsGenieDispatchConfig                                                        | None`                                                        | An optional dispatch configuration used to configure how alerts are routed, if None is provided a default internal dispatch type of Console will be used to log alerts to the conosole of the scouter server.  | `config.dispatch_config -> SlackDispatchConfig()` |
| schedule            | `str                 | CommonCrons                                                                   | None`                                                        | Schedule to run drift detection job. Defaults to daily at midnigh. You can use the builtin CommonCron options or specify your own custom cron.                  | `config.schedule → CommonCrons.Every6Hours` |
| features_to_monitor | `list[str]`                                        | List of features to monitor. Defaults to empty list, which means all features | `config.features_to_monitor → ['feature_1, feature_2, ...']` |
| psi_threshold        | `float`              | Defaults to the industry standard of 0.25. If one of your monitored features surpass the psi_threshold, an alert will be sent.                                   | `config.psi_threshold → 0.25`                                |

### Properties

| Property              | Type        | Description                                                                         | Example                              |
|-----------------------|-------------|-------------------------------------------------------------------------------------|--------------------------------------|
| `dispatch_type`       | `str`       | String representation of what type of dispatch are you using to send alerts.        | `config.dispatch_type` → `"Slack"`   |
| `dispatch_Config`     | `SlackDispatchConfig | OpsGenieDispatchConfig                                                        | None`      | Dispatch configuration used to configure how alerts are routed.                                 | `config.dispatch_config -> SlackDispatchConfig()`   |
| `schedule`            | `str`       | The schedule that is used to determine when your drift detecion job should run.     | `config.schedule` → `"0 0 0 * * SUN"` |
| `features_to_monitor` | `list[str]` | List of features to monitor.                                                        | `config.features_to_monitor → ['feature_1, feature_2, ...']`        |
| `psi_threshold`       | `float`     | If one of your monitored features surpass the psi_threshold, an alert will be sent. | `config.psi_threshold → 0.25`         |

