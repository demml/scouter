# Spc Feature Drift Profile

---

!!! info "scouter.drift.SpcFeatureDriftProfile"
The `SpcFeatureDriftProfile` is assigned to each feature when creating a `SpcDriftProfile`. The `SpcFeatureDriftProfile` will contain information about your features's center and zone control limits.

---


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