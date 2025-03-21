# PSI Drift Configuration

---

!!! info "scouter.drift.PsiDriftConfig"
All models that create a `PsiDriftProfile` will require a `PsiDriftConfig` object.

---

```py
from scouter.alert import PsiAlertConfig
from scouter.drift import PsiDriftConfig

PsiDriftConfig(
    name="wine_model",
    repository="wine_model",
    version="0.0.1",
    alert_config=PsiAlertConfig()
)
```

## Parameters

| Parameter       | Type             | Description                                                                                                          | Example |
|---------------|------------------|----------------------------------------------------------------------------------------------------------------------|---------|
| name        | `str`            | The name of the model or dataset being monitored. Defaults to '\_\_missing\_\_' if not provided.                     | `config.name → "wine_model"` |
| repository  | `str`            | The repository where the model or dataset is stored. Defaults to '\_\_missing\_\_' if not provided.                  | `config.repository → "wine_model"` |
| version     | `str`            | The version of the model or dataset being monitored. Defaults to '0.0.1' if not provided.                            | `config.version → "0.0.1"` |
| alert_config | `PsiAlertConfig` | Configuration for alerting when drift is detected. Defaults to the default implementation of PsiAlertConfig if not provided. | `config.alert_config → *Instance of PsiAlertConfig*` |
| targets     | `list[str]`      | List of target features, typically the dependent variable(s).                                                        | `config.targets → ["churn"]` |
| config_path | `Optional[Path]` | Path to a pre existing PsiDriftConfig. Defaults to None if not provided                                             | `config.config_path → Path("/configs/drift.yaml")` |



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

### `load_from_json_file()` _(static method)_
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
