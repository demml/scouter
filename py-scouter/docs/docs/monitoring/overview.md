# Monitoring

Monitoring has been a long-discussed topic in machine learning and the management of machine learning systems. However, the tools and methods for monitoring are still in their infancy. `Scouter` allows you to get up and running with monitoring in a few lines of code.

## Getting Started

To begin monitoring, you must first create a `drift profile` from your data. This profile will be used as a source of truth when comparing new data to the original data. 

### Creating a Drift Profile

Creating a `DriftProfile` is the first step to setting up monitoring and is as simple as:

1. Get your randomized data (can be a 2D numpy array, pandas dataframe or polars dataframe).
2. Create a `DriftConfig` object.
3. Instantiate a `Drifter` object and create a `DriftProfile` via the `create_drift_profile` method.
4. Save the profile to disk or send it to `scouter-server`.

### Example

```python
from scouter import Drifter, DriftConfig

# Assume we have some data (numpy array, pandas dataframe, polars dataframe)
data = generate_my_data()

# (1) Create a drift config
config = DriftConfig(
        name="model", # this is usually your model name
        repository="scouter", # repository your model belongs to
        version="0.1.0", # current version of your model
    )

# (2) Instantiate the Drifter
drifter = Drifter()

# (3) Create a drift profile
profile = drifter.create_drift_profile(data, config)

# this profile can be saved to disk or sent to scouter-server for storage
print(profile)
```

```json
{
  "features": {
    "feature_1": {
      "id": "col_0",
      "center": -4.139930289046504,
      "one_ucl": -2.0997890884791675,
      "one_lcl": -6.18007148961384,
      "two_ucl": -0.05964788791183118,
      "two_lcl": -8.220212690181176,
      "three_ucl": 1.980493312655505,
      "three_lcl": -10.260353890748512,
      "timestamp": "2024-06-26T20:43:27.957150"
    },
    "feature_2": {
      "id": "col_11",
      "center": 9.736,
      "one_ucl": 15.325429018306778,
      "one_lcl": 4.146570981693224,
      "two_ucl": 20.914858036613552,
      "two_lcl": -1.4428580366135524,
      "three_ucl": 26.50428705492033,
      "three_lcl": -7.032287054920328,
      "timestamp": "2024-06-26T20:43:27.957235"
    },
    "Feature_3": {
      "id": "col_9",
      "center": -3.9852524079139835,
      "one_ucl": -2.029949081211379,
      "one_lcl": -5.940555734616588,
      "two_ucl": -0.07464575450877531,
      "two_lcl": -7.895859061319191,
      "three_ucl": 1.8806575721938286,
      "three_lcl": -9.851162388021795,
      "timestamp": "2024-06-26T20:43:27.957235"
    },
    "target": {
      "id": "target",
      "center": 4.987,
      "one_ucl": 7.562620467955954,
      "one_lcl": 2.4113795320440463,
      "two_ucl": 10.138240935911908,
      "two_lcl": -0.16424093591190747,
      "three_ucl": 12.713861403867861,
      "three_lcl": -2.7398614038678613,
      "timestamp": "2024-06-26T20:43:27.957235"
    }
  },
  "config": {
    "sample_size": 25,
    "sample": true,
    "name": "model",
    "repository": "scouter",
    "version": "0.1.0",
    "schedule": "0 0 0 * * *",
    "alert_rule": {
      "process": {
        "rule": "8 16 4 8 2 4 1 1"
      },
      "percentage": null
    }
  }
}
```

### What is a Drift Profile?

A `DriftProfile` is a collection of feature statistics along with a monitoring configuration that will serve as the source of truth for your monitoring. It contains two main components:

- **Features**: A dictionary object containing a feature name and corresponding `FeatureDriftProfile` object.
- **Config**: A `DriftConfig` object containing information about how you want drift calculated (sample size, schedule, alert rules, etc.). More on this later.


### How to Generate Alerts

Once you have a `DriftProfile`, you can use it to generate alerts when new data is passed through the `Drifter` object. The following steps can be used to generate alerts:

1. Get your new data (can be a 2D numpy array, pandas dataframe or polars dataframe).
2. Load the `DriftProfile`.
3. Compute the drift using the `Drifter` object.
4. Generate alerts using the `Drifter` object.
5. Send the alerts where you need them to go.

**Note - When using the `scouter-server`, all of the above is handled for you. You only need to send the drift profile and new data to the server.**

### Example

```python
from scouter import Drifter

new_data = generate_new_data()

# Check for drift (use the original drift profile)
drift_map = drifter.compute_drift(data, profile)

### this will return a DriftMap object. We need to convert it to a numpy array for alert generation
drift_array, features = drift_map.to_py()

# Generate alerts
feature_alerts = drifter.generate_alerts(
        drift_array, features, profile.config.alert_rule
    )

print(feature_alerts)
```

```json
{
  "features": {
    "feature_1": {
      "feature": "feature_1",
      "alerts": [],
      "indices": {}
    },
    "feature_2": {
      "feature": "feature_2",
      "alerts": [],
      "indices": {}
    },
    "Feature_3": {
      "feature": "Feature_3",
      "alerts": [
        {
          "kind": "Consecutive",
          "zone": "Zone 1"
        }
      ],
      "indices": {
        "1": [
          [
            9,
            17
          ]
        ]
      }
    },
    "target": {
      "feature": "target",
      "alerts": [],
      "indices": {}
    }
  }
}
```

For more information on the theory and application of alerting, see the [alerting](./alerting.md) section.

