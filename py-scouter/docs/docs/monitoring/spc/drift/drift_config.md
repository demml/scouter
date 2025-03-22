# SPC Drift Configuration

---

!!! info "scouter.drift.SpcDriftConfig"
All models that create a `SpcDriftProfile` will require a `SpcDriftConfig` object.

---

```py
from scouter.alert import SpcAlertConfig
from scouter.drift import SpcDriftConfig

SpcDriftConfig(
    name="wine_model",
    repository="wine_model",
    version="0.0.1",
    alert_config=SpcAlertConfig(),
    sample_size=1000
)
```

## Parameters

| Parameter    | Type             | Description                                                                                                                    | Example                                              |
|--------------|------------------|--------------------------------------------------------------------------------------------------------------------------------|------------------------------------------------------|
| name         | `str`            | The name of the model or dataset being monitored. Defaults to '\_\_missing\_\_' if not provided.                               | `config.name → "wine_model"`                         |
| repository   | `str`            | The repository where the model or dataset is stored. Defaults to '\_\_missing\_\_' if not provided.                            | `config.repository → "wine_model"`                   |
| version      | `str`            | The version of the model or dataset being monitored. Defaults to '0.0.1' if not provided.                                      | `config.version → "0.0.1"`                           |
| alert_config | `SpcAlertConfig` | Configuration for alerting when drift is detected. Defaults to the default implementation of SpcAlertConfig if not provided.   | `config.alert_config → *Instance of SpcAlertConfig*` |
| targets      | `list[str]`      | List of target features, typically the dependent variable(s).                                                                  | `config.targets → ["churn"]`                         |
| config_path  | `Optional[Path]` | Path to a pre existing SpcDriftConfig. Defaults to None if not provided                                                        | `config.config_path → Path("/configs/drift.yaml")`   |
| sample       | `bool`           | Specifies whether sampling should be applied when calculating SPC metrics. Defaults to True.                                   | `config.sample → True`                               |
| sample_size       | `int`            | Defines the number of data points to include in the sample when sampling is enabled for SPC metric computation. Defaults to 25 | `config.sample → True`                               |



## Properties


| Property       | Type            | Description                                                                                                                                        | Example                                              |
|----------------|-----------------|----------------------------------------------------------------------------------------------------------------------------------------------------|------------------------------------------------------|
| `name`         | `str`           | The name of the model or dataset being monitored.                                                                                                  | `config.name` → `"wine_model"`                       |
| `repository`   | `str`           | The repository where the model or dataset is stored.                                                                                               | `config.repository` → `"wine_model"`                 |
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
    - **`repository`** (`Optional[str]`): The repository name, if updating.
    - **`name`** (`Optional[str]`): The new name for the configuration, if provided.
    - **`version`** (`Optional[str]`): The version to set, if specified.
    - **`targets`** (`Optional[List[str]]`): A list of target identifiers, if updating.
    - **`alert_config`** (`Optional[SpcAlertConfig]`): The alert configuration, if provided.
    - **`sample`** (`Optional[bool]`): Opt in or out of the sampling strategy.
    - **`sample_size`** (`Optional[int]`): Adjust the sample size.
- **Returns:** `None`
- **Return Type:** `None`