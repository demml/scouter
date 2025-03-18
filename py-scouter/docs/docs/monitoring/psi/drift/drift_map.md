# PSI Drift Map

---

!!! info "scouter.drift.PsiDriftMap"
The `PsiDriftMap` is returned from the offline **(non scouter server)** `compute_drift` function found on the `Drifter` object. If you're calculating drift offline and need to view the reported drift, the `PsiDriftMap` object contains the information you're looking for.

---


## Properties


| Property       | Type             | Description                                                                                                                                        | Example                                                                    |
|----------------|------------------|----------------------------------------------------------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------|
| `name`         | `str`            | The name of the model or dataset being monitored.                                                                                                  | `map.name` → `"wine_model"`                                                |
| `repository`   | `str`            | The repository where the model or dataset is stored.                                                                                               | `map.repository` → `"wine_model"`                                          |
| `version`      | `str`            | The version of the model or dataset being monitored.                                                                                               | `map.version` → `"0.0.1"`                                                  |
| `features`  | `Dict[str, float]`     | Returns dictionary of features and their reported drift, if any. | `map.features` → `{'feature_1': 0.0, 'feature_0': 0.1, 'feature_2': 0.04}` |

## Methods

### `model_dump_json()`
Serializes the `PsiDriftMap` instance to a JSON string.

- **Parameters:** None
- **Returns:** A JSON string representation of the instance.
- **Return Type:** `str`

---

### `save_to_json()`
Save drift profile to json file.

- **Parameters:**
    - **`path`** (`Optional[Path]`): Optional path to save the drift map. If None, outputs to drift_map.json.
- **Returns:** `None`
- **Return Type:** `None`

---

### `model_validate_json()` _(static method)_
Validate a JSON string representation of a saved PSI drift map.

- **Parameters:**
    - **`json_string`** (`str`): PsiDriftMap in JSON string format.
- **Returns:** `PsiDriftMap`
- **Return Type:** `PsiDriftMap`
