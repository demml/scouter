# Spc Drift Map

---

!!! info "scouter.drift.SpcDriftMap"
The `SpcDriftMap` is returned from the offline **(non scouter server)** `compute_drift` function found on the `Drifter` object. If you're calculating drift offline and need to view the reported drift, the `SpcDriftMap` object contains the information you're looking for.

---


## Properties


| Property       | Type             | Description                                                                                                                                        | Example                                                                               |
|----------------|------------------|----------------------------------------------------------------------------------------------------------------------------------------------------|---------------------------------------------------------------------------------------|
| `name`         | `str`            | The name of the model or dataset being monitored.                                                                                                  | `map.name` → `"wine_model"`                                                           |
| `repository`   | `str`            | The repository where the model or dataset is stored.                                                                                               | `map.repository` → `"wine_model"`                                                     |
| `version`      | `str`            | The version of the model or dataset being monitored.                                                                                               | `map.version` → `"0.0.1"`                                                             |
| `features`  | `Dict[str, SpcFeatureDrift]`     | Returns dictionary of features and their reported drift, if any. | `map.features` → `{'feature_1': *instance of SpcFeatureDrift*, 'feature_0': *instance of SpcFeatureDrift* 'feature_2': *instance of SpcFeatureDrift*}` |

## Methods

### `model_dump_json()`
Serializes the `SpcDriftMap` instance to a JSON string.

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
Validate a JSON string representation of a saved SPC drift map.

- **Parameters:**
    - **`json_string`** (`str`): SpcDriftMap in JSON string format.
- **Returns:** `SpcDriftMap`
- **Return Type:** `SpcDriftMap`

---

### `to_numpy()`
Convert `SpcDriftMap` to a tuple of numpy arrays. (sample_array, drift_array, list of features)

- **Parameters:** None
- **Returns:** Return `SpcDriftMap` as a tuple of sample_array, drift_array and list of features.
- **Return Type:** `SpcDriftMap`