# Spc Drift Profile

The `SpcDriftProfile` serves as the core component for monitoring model drift in production.

## Properties


| Property       | Type                                | Description                                                            | Example                                                                         |
|----------------|-------------------------------------|------------------------------------------------------------------------|---------------------------------------------------------------------------------|
| `scouter_version`         | `str`                               | The version of scouter that was used to create your SPC drift profile. | `spc_profile.scouter_version` → `"1.0.0"`                                       |
| `features`   | `dict[str, SpcFeatureDriftProfile]` | A mapping of feature names to their respective drift profiles.         | `spc_profile.features['feature_name']` → `*Instance of SpcFeatureDriftProfile*` |
| `config`      | `SpcDriftConfig`                    | The drift config defined at the time of profile creationg.             | `spc_profile.config` → `*Instance of SpcDriftConfig*`                           |

## Methods

### `model_dump_json()`
Serializes the `SpcDriftProfile` instance to a JSON string.

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
    - **`sample`** (`Optional[bool]`): Whether to use sampling or not.
    - **`sample_size`** (`Optional[bool]`): Size of the samples you want to use.
    - **`alert_config`** (`Optional[SpcAlertConfig]`): Instance of `SpcAlertConfig`
- **Returns:** `None`
- **Return Type:** `None`

---

### `model_validate_json()` _(static method)_
Validate a JSON string representation of a saved profile.

- **Parameters:**
    - **`json_string`** (`str`): `SpcDriftProfile` in JSON string format.
- **Returns:** `SpcDriftProfile`
- **Return Type:** `SpcDriftProfile`

---

### `from_file()` _(static method)_
Load a `SpcDriftProfile` from file.

- **Parameters:**
    - **`path`** (`Path`): Path to the file.
- **Returns:** `SpcDriftProfile`
- **Return Type:** `SpcDriftProfile`

---

### `model_validate()` _(static method)_
Validate a dict representation of a saved profile.

- **Parameters:**
    - **`data`** (`Dict[str, Any]`): dict representation of your `SpcDriftProfile`.
- **Returns:** `SpcDriftProfile`
- **Return Type:** `SpcDriftProfile`


## Features

### `SpcFeatureDriftProfile`
The `SpcFeatureDriftProfile` is assigned to each feature when creating a `SpcDriftProfile`. The `SpcFeatureDriftProfile` will contain information about your features's center and zone control limits.



## Properties


| Property    | Type      | Description                         | Example                                        |
|-------------|-----------|-------------------------------------|------------------------------------------------|
| `id`        | `str`     | The name of the feature.            | `profile.name` → `"feature_1"`                 |
| `center`    | `float`   | Mean of the sample for the feature. | `profile.center` → `.6`                        |
| `one_ucl`   | `float`   | Zone 1 upper control limit.         | `profile.one_ucl` → `.1`                       |
| `one_lcl`   | `float`   | Zone 1 lower control limit.         | `profile.one_lcl` → `.2`                       |
| `two_ucl`   | `float`   | Zone 2 upper control limit.         | `profile.two_ucl` → `.3`                       |
| `two_lcl`   | `float`   | Zone 2 lower control limit.         | `profile.two_lcl` → `.4`                       |
| `three_ucl` | `float`   | Zone 3 upper control limit.         | `profile.three_ucl` → `.5`                     |
| `three_lcl` | `float`   | Zone 3 lower control limit.         | `profile.three_lcl` → `.6`                     |
| `timestamp` | `str`     | Time of creation.                   | `profile.timestamp` → `"2025-03-13T14:30:00Z"` |