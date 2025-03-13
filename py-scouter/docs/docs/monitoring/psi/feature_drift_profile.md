# PSI Feature Drift Profile

---

!!! info "scouter.drift.PsiFeatureDriftProfile"
The `PsiFeatureDriftProfile` is assigned to each feature when creating a `PsiDriftProfile`. The `PsiFeatureDriftProfile` will contain information about the bins constructed per the decile formula.

---


## Properties


| Property    | Type        | Description                                          | Example                                  |
|-------------|-------------|------------------------------------------------------|------------------------------------------|
| `id`        | `str`       | The name of the feature.                             | `profile.name` → `"feature_1"`           |
| `bins`      | `list[Bin]` | List of the bins assigned to the feature.            | `profile.bins` → `"[*Instance of Bin*]"` |
| `timestamp` | `str`       | Time of creation.                                    | `profile.timestamp` → `"2025-03-13T14:30:00Z"`                |
| `bin_type`  | `BinType`       | A bin can be either Categorical, Numeric, or Binary. | `profile.bin_type` → `BinType.Numeric`                |