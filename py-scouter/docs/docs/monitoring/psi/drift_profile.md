# PSI Drift Profile


The `PsiDriftProfile` is the returned object from the `create_drift_profile` function found on the `Drifter` object. This object contains the drift profile for your data and is used as the source of truth for drift detection.



## Properties

| Property       | Type                                | Description                                                            | Example                                                                       |
|----------------|-------------------------------------|------------------------------------------------------------------------|-------------------------------------------------------------------------------|
| `scouter_version`         | `str`                               | The version of scouter that was used to create your PSI drift profile. | `psi_profile.scouter_version` → `"1.0.0"`                                     |
| `features`   | `dict[str, PsiFeatureDriftProfile]` | A mapping of feature names to their respective drift profiles.         | `psi_profile.features['feature_name']` → `*Instance of PsiFeatureDriftProfile*` |
| `config`      | `PsiDriftConfig`                               | The drift config defined at the time of profile creationg.             | `psi_profile.config` → `*Instance of PsiDriftConfig*`                                       |

## Methods

### `model_dump_json()`
Serializes the `PsiDriftProfile` instance to a JSON string.

- **Parameters:** None
- **Returns:** A JSON string representation of the instance.
- **Return Type:** `str`

---

### `model_dump()`
Return dictionary representation of the drift profile.

- **Parameters:** None
- **Returns:** `dict[str, Any]` representation of the instance.
- **Return Type:** `dict[str, Any]`

---

### `save_to_json()`
Save drift profile to json file.

- **Parameters:**
    - **`path`** (`Optional[Path]`): Optional path to save the drift profile. If None, outputs to drift_profile.json.
- **Returns:** `None`
- **Return Type:** `None`

---

### `update_config_args()`
Inplace operation that updates config args.

- **Parameters:**
    - **`space`** (`Optional[str]`): Name of the model space.
    - **`name`** (`Optional[str]`): Name of the model.
    - **`version`** (`Optional[str]`): Version of the model.
    - **`targets`** (`Optional[str]`): Target(s) of the model / Dependant variable(s).
    - **`alert_config`** (`Optional[PsiAlertConfig]`): Instance of `PsiAlertConfig`.
- **Returns:** `None`
- **Return Type:** `None`

---

### `model_validate_json()` _(static method)_
Validate a JSON string representation of a saved profile.

- **Parameters:**
    - **`json_string`** (`str`): PsiDriftProfile in JSON string format.
- **Returns:** `PsiDriftProfile`
- **Return Type:** `PsiDriftProfile`

---

### `from_file()` _(static method)_
Load a `PsiDriftProfile` from file.

- **Parameters:**
    - **`path`** (`Path`): Path to the file.
- **Returns:** `PsiDriftProfile`
- **Return Type:** `PsiDriftProfile`

---

### `model_validate()` _(static method)_
Validate a dict representation of a saved profile.

- **Parameters:**
    - **`data`** (`Dict[str, Any]`): dict representation of your PsiDriftProfile.
- **Returns:** `PsiDriftProfile`
- **Return Type:** `PsiDriftProfile`

## Features

### `PsiFeatureDriftProfile`
The `PsiFeatureDriftProfile` is assigned to each feature when creating a `PsiDriftProfile`. The `PsiFeatureDriftProfile` will contain information about the bins constructed per the decile formula.



### Properties


| Property    | Type        | Description                                          | Example                                  |
|-------------|-------------|------------------------------------------------------|------------------------------------------|
| `id`        | `str`       | The name of the feature.                             | `profile.name` → `"feature_1"`           |
| `bins`      | `list[Bin]` | List of the bins assigned to the feature.            | `profile.bins` → `"[*Instance of Bin*]"` |
| `timestamp` | `str`       | Time of creation.                                    | `profile.timestamp` → `"2025-03-13T14:30:00Z"`                |
| `bin_type`  | `BinType`       | A bin can be either Categorical, Numeric, or Binary. | `profile.bin_type` → `BinType.Numeric`                |