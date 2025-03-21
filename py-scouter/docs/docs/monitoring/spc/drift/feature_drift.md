# Spc Feature Drift

---

!!! info "scouter.drift.SpcFeatureDrift"
The `SpcFeatureDrift` is an item stored in the `features` map within the `SpcDriftMap`.

---

## Properties

| Property    | Type          | Description                                                                                                                                                                                                                                           | Example                    |
|-------------|---------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|----------------------------|
| `samples`   | `List[float]` | The mean of the feature's values over a window of samples. This data is gathered during drift detection, where the data is divided into chunks, and the mean of each chunk is computed. It provides a snapshot of the feature's distribution over time. | `drift.samples` → `[.2,.3]` |
| `drift`     | `List[float]` | A vector of drift values for a feature, representing changes in the distribution over time. Drift is calculated by comparing observed sample means to a baseline, helping to detect shifts in the feature's behavior.                                      | `drift.drift` → `[.2,.3]`        |
