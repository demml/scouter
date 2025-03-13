# PSI Drift Profile

---

!!! info "scouter.drift.PsiDriftProfile"
The `PsiDriftProfile` serves as the core component for monitoring model drift in production.

---


## Properties


| Property       | Type                                | Description                                                            | Example                                                                       |
|----------------|-------------------------------------|------------------------------------------------------------------------|-------------------------------------------------------------------------------|
| `scouter_version`         | `str`                               | The version of scouter that was used to create your PSI drift profile. | `psi_profile.scouter_version` → `"1.0.0"`                                     |
| `features`   | `dict[str, PsiFeatureDriftProfile]` | A mapping of feature names to their respective drift profiles.         | `psi_profile.features['feature_name']` → `*Instance of PsiFeatureDriftProfile*` |
| `config`      | `PsiDriftConfig`                               | The drift config defined at the time of profile creationg.             | `psi_profile.config` → `*Instance of PsiDriftConfig*`                                       |

## Methods

### `model_dump_json()`
Serializes the `PsiDriftConfig` instance to a JSON string.

- **Parameters:** None
- **Returns:** A JSON string representation of the instance.
- **Return Type:** `str`

---

### `model_dump()`
Return dictionary representation of the drift profile.

- **Parameters:** None
- **Returns:** `dict[str]` representation of the instance.
- **Return Type:** `dict[str]`

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
    - **`repository`** (`Optional[str]`): Name of the model repository.
    - **`name`** (`Optional[str]`): Name of the model.
    - **`version`** (`Optional[str]`): Version of the model.
    - **`targets`** (`Optional[str]`): Target(s) of the model / Dependant variable(s).
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
