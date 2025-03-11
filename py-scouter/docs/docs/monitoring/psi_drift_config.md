# PSI Drift Configuration

All models that create a `PsiDriftProfile` will require a `PsiDriftConfig` object.

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

| Parameter       | Type             | Description                                                                                                                  | Example |
|---------------|------------------|------------------------------------------------------------------------------------------------------------------------------|---------|
| name        | `str`            | The name of the model or dataset being monitored. Defaults to '\_\_missing\_\_' if not provided.                             | `config.name → "wine_model"` |
| repository  | `str`            | The repository where the model or dataset is stored. Defaults to '\_\_missing\_\_' if not provided.                          | `config.repository → "wine_model"` |
| version     | `str`            | The version of the model or dataset being monitored. Defaults to '0.0.1' if not provided.                                    | `config.version → "0.0.1"` |
| alert_config | `PsiAlertConfig` | Configuration for alerting when drift is detected. Defaults to the default implementation of PsiAlertConfig if not provided. | `config.alert_config → *Instance of PsiAlertConfig*` |
| targets     | `list[str]`      | List of target features, typically the dependent variable(s).                                                                | `config.targets → ["churn"]` |
| config_path | `Optional[Path]` | Path to a saved drift configuration file. Defaults to None if not provided                                                   | `config.config_path → Path("/configs/drift.yaml")` |



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

| Method                     | Description                                                                 | Parameters                                                                                                                                                             | Returns                             | Return Type                |
|----------------------------|-----------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------|-------------------------------------|----------------------------|
| `load_from_json_file()`     | Loads a `PsiDriftConfig` instance from a JSON file.                         | **`path`** (`Path`): The path to the JSON configuration file. This is required to locate and read the configuration file from disk.                                                                                                                                  | A `PsiDriftConfig`. | `PsiDriftConfig`           |
| `model_dump_json()`         | Serializes the `PsiDriftConfig` instance to a JSON string.                  | None                                                                                                                                                                   | A JSON string representation of the instance. | `str`                      |
| `update_config_args()`      | Updates the configuration of the instance with new values.                 | `repository` (`Option<String>`), `name` (`Option<String>`), `version` (`Option<String>`), `targets` (`Option<Vec<String>>`), `alert_config` (`Option<PsiAlertConfig>`) | `Result<(), ScouterError>` | `Result<(), ScouterError>` |


    pub fn update_config_args(
        &mut self,
        repository: Option<String>,
        name: Option<String>,
        version: Option<String>,
        targets: Option<Vec<String>>,
        alert_config: Option<PsiAlertConfig>,
    ) -> Result<(), ScouterError> {

    def update_config_args(
        self,
        repository: Optional[str] = None,
        name: Optional[str] = None,
        version: Optional[str] = None,
        feature_map: Optional[FeatureMap] = None,
        targets: Optional[List[str]] = None,
        alert_config: Optional[PsiAlertConfig] = None,
    ) -> None: