# SPC Drift Configuration

All models that create a `SpcDriftProfile` will require a `SpcDriftConfig` object.

```py
from scouter.alert import SpcAlertConfig
from scouter.drift import SpcDriftConfig

SpcDriftConfig(
    name="wine_model",
    space="wine_model",
    version="0.0.1",
    alert_config=SpcAlertConfig(),
    sample_size=1000
)
```

### Parameters

| Parameter    | Type             | Description                                                                                                                    | Example                                              |
|--------------|------------------|--------------------------------------------------------------------------------------------------------------------------------|------------------------------------------------------|
| name         | `str`            | The name of the model or dataset being monitored. Defaults to '\_\_missing\_\_' if not provided.                               | `config.name → "wine_model"`                         |
| space   | `str`            | The space where the model or dataset is stored. Defaults to '\_\_missing\_\_' if not provided.                            | `config.space → "wine_model"`                   |
| version      | `str`            | The version of the model or dataset being monitored. Defaults to '0.0.1' if not provided.                                      | `config.version → "0.0.1"`                           |
| alert_config | `SpcAlertConfig` | Configuration for alerting when drift is detected. Defaults to the default implementation of SpcAlertConfig if not provided.   | `config.alert_config → *Instance of SpcAlertConfig*` |
| targets      | `list[str]`      | List of target features, typically the dependent variable(s).                                                                  | `config.targets → ["churn"]`                         |
| config_path  | `Optional[Path]` | Path to a pre existing SpcDriftConfig. Defaults to None if not provided                                                        | `config.config_path → Path("/configs/drift.yaml")`   |
| sample       | `bool`           | Specifies whether sampling should be applied when calculating SPC metrics. Defaults to True.                                   | `config.sample → True`                               |
| sample_size       | `int`            | Defines the number of data points to include in the sample when sampling is enabled for SPC metric computation. Defaults to 25 | `config.sample → True`                               |



### Properties


| Property       | Type            | Description                                                                                                                                        | Example                                              |
|----------------|-----------------|----------------------------------------------------------------------------------------------------------------------------------------------------|------------------------------------------------------|
| `name`         | `str`           | The name of the model or dataset being monitored.                                                                                                  | `config.name` → `"wine_model"`                       |
| `space`   | `str`           | The space where the model or dataset is stored.                                                                                               | `config.space` → `"wine_model"`                 |
| `version`      | `str`           | The version of the model or dataset being monitored.                                                                                               | `config.version` → `"0.0.1"`                         |
| `feature_map`  | `FeatureMap`    | When a non-numeric covariate is detected, each unique value is assigned a corresponding numeric value. This mapping is represented by feature_map. | `config.feature_map` → `Instance of FeatureMap`      |
| `targets`      | `list[str]`     | List of target features, typically the dependent variable(s).                                                                                      | `config.targets` → `label`                           |
| `alert_config` | `SpclertConfig` | Configuration for alerting when drift is detected.                                                                                                 | `config.alert_config` → `Instance of SpcAlertConfig` |
| `drift_type`   | `DriftType`     | Type of drift profile.                                                                                                                             | `config.drift_type` → `DriftType.Spc`                |
| `sample_size`   | `int`           | Defines the number of data points to include in the sample when sampling is enabled for SPC metric computation.                                                                                                                       | `config.sample_size` → `1000`                        |
| `sample`   | `bool`          | Specifies whether sampling should be applied when calculating SPC metrics.                                                                                                                          | `config.sample` → `True`                             |



## Methods

### `load_from_json_file()` _(static method)_
Loads a `SpcDriftConfig` instance from a JSON file.

- **Parameters:**
    - **`path`** (`Path`): The path to the JSON configuration file. This is required to locate and read the configuration file from disk.
- **Returns:** A `SpcDriftConfig` instance.
- **Return Type:** `SpcDriftConfig`

---

### `model_dump_json()`
Serializes the `SpcDriftConfig` instance to a JSON string.

- **Parameters:** None
- **Returns:** A JSON string representation of the instance.
- **Return Type:** `str`

---

### `update_config_args()`
Updates the configuration of the instance with new values.

- **Parameters:**
    - **`space`** (`Optional[str]`): The space name, if updating.
    - **`name`** (`Optional[str]`): The new name for the configuration, if provided.
    - **`version`** (`Optional[str]`): The version to set, if specified.
    - **`targets`** (`Optional[List[str]]`): A list of target identifiers, if updating.
    - **`alert_config`** (`Optional[SpcAlertConfig]`): The alert configuration, if provided.
    - **`sample`** (`Optional[bool]`): Opt in or out of the sampling strategy.
    - **`sample_size`** (`Optional[int]`): Adjust the sample size.
- **Returns:** `None`
- **Return Type:** `None`

## Alert Configuration

An `AlertConfig` can also be provided to the `SpcDriftConfig` to specify how you and your team want to be alerted in the event of model drift. The `SpcAlertConfig` class allows you to configure the alerting mechanism, including the dispatch method (e.g., Slack, OpsGenie) and the schedule for drift detection jobs.

```py
from scouter.alert import SpcAlertConfig, OpsGenieDispatchConfig, SpcAlertRule
from scouter.types import CommonCrons

SpcAlertConfig(
    rule=SpcAlertRule(rule="16 32 4 8 2 4 1 1"),
    dispatch_config=OpsGenieDispatchConfig(team='the-ds-team'),
    schedule=CommonCrons.EveryDay,
    features_to_monitor=['feature_1', 'feature_2', ...],
)
```


### Parameters

| Parameter           | Type                 | Description                                                                   | Example                                                      |
|---------------------|----------------------|-------------------------------------------------------------------------------|--------------------------------------------------------------|
| dispatch_config     | `SlackDispatchConfig | OpsGenieDispatchConfig                                                        | None`                                                        | An optional dispatch configuration used to configure how alerts are routed, if None is provided a default internal dispatch type of Console will be used to log alerts to the conosole of the scouter server.  | `config.dispatch_config -> SlackDispatchConfig()` |
| schedule            | `str                 | CommonCrons                                                                   | None`                                                        | Schedule to run drift detection job. Defaults to daily at midnigh. You can use the builtin CommonCron options or specify your own custom cron.                  | `config.schedule → CommonCrons.Every6Hours` |
| features_to_monitor | `list[str]`                                        | List of features to monitor. Defaults to empty list, which means all features | `config.features_to_monitor → ['feature_1, feature_2, ...']` |
| rule                | `SpcAlertRule`              | Defines the conditions for triggering alerts based on patterns observed in the control chart. Defaults to "8 16 4 8 2 4 1 1", where each digit specifies a threshold for detecting instability within each control zone (Zone 1 to Zone 4). Can be customized for more or less sensitivity.                       | `config.rule → *Instance of SpcAlertRule*`                   |

### Properties

| Property              | Type        | Description                                                                         | Example                              |
|-----------------------|-------------|-------------------------------------------------------------------------------------|--------------------------------------|
| `dispatch_type`       | `str`       | String representation of what type of dispatch are you using to send alerts.        | `config.dispatch_type` → `"Slack"`   |
| `dispatch_Config`     | `SlackDispatchConfig | OpsGenieDispatchConfig                                                        | None`      | Dispatch configuration used to configure how alerts are routed.                                 | `config.dispatch_config -> SlackDispatchConfig()`   |
| `schedule`            | `str`       | The schedule that is used to determine when your drift detecion job should run.     | `config.schedule` → `"0 0 0 * * SUN"` |
| `features_to_monitor` | `list[str]` | List of features to monitor.                                                        | `config.features_to_monitor → ['feature_1, feature_2, ...']`        |
| `rule`       | `SpcAlertRule`     | Defines the conditions for triggering alerts based on patterns observed in the control chart. Defaults to "8 16 4 8 2 4 1 1", where each digit specifies a threshold for detecting instability within each control zone (Zone 1 to Zone 4). Can be customized for more or less sensitivity.                       | `config.rule → *Instance of SpcAlertRule*`                   |

## AlertRule

The `SpcAlertRule` class is used to define the conditions for triggering alerts based on patterns observed in the control chart. The rule is represented as a string of digits, where each digit specifies a threshold for detecting instability within each control zone (Zone 1 to Zone 4). The default rule is "8 16 4 8 2 4 1 1", which can be customized for more or less sensitivity.

```py
rom scouter.alert import SpcAlertRule, AlertZone

SpcAlertRule(
    rule="8 16 4 8 2 4 1 1",
    zones_to_monitor=[AlertZone.Zone1, AlertZone.Zone2, AlertZone.Zone3, AlertZone.Zone4]
)
```


### Parameters

| Parameter           | Type              | Description                                                                 | Example                                 |
|---------------------|-------------------|-----------------------------------------------------------------------------|-----------------------------------------|
| rule                | `str`             | Rule to use for alerting. Eight digit integer string. Defaults to '8 16 4 8 2 4 1 1 | `alert_rule.rule -> "8 16 4 8 2 4 1 1"` |
| zones_to_monitor    | `list[AlertZone]` |  List of zones to monitor. Defaults to all zones.                  | `alert_rule.zones → [AlertZone.Zone1, AlertZone.Zone2]`                 |

### Properties

| Property              | Type        | Description                                                                         | Example                              |
|-----------------------|-------------|-------------------------------------------------------------------------------------|--------------------------------------|
| rule                | `str`             | Rule to use for alerting. Eight digit integer string. Defaults to '8 16 4 8 2 4 1 1 | `alert_rule.rule -> "8 16 4 8 2 4 1 1"` |
| zones_to_monitor    | `list[AlertZone]` |  List of zones to monitor. Defaults to all zones.                  | `alert_rule.zones → [AlertZone.Zone1, AlertZone.Zone2]`                 |