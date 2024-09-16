<h1 align="center">
  <br>
  <img src="https://github.com/demml/scouter/blob/main/images/scouter-logo.png?raw=true"  width="1181" alt="scouter logo"/>
  <br>
</h1>

<h2 align="center"><b>Coming Soon: Observability for Machine Learning</b></h2>


<h2 align="center"><a href="https://demml.github.io/scouter/">Doc Site</h2>

# Scouter

Scouter aims to make model monitoring and data profiling simple. No gotchas or extra work needed to get started.

Highlights:
 - **Fast**: The core logic is written in Rust and exposed as a Python package via Maturin and PyO3. This means super fast array processing and asynchronous execution for small and large datasets.
 - **Simple**: Scouter's python interface is designed to be simple and easy to use. You can get started with just a few lines of code.
 - **Server-Integration**: Scouter can be integrated with the [scouter-server](https://github.com/demml/scouter-server) to provide a centralized monitoring and alerting system for your models. **Note** that the server is still in development and not yet ready for production use.
 - **Data Profiling**: Scouter can be used to profile your data and provide insights into the distributions and quality of your data.
 - **Model Monitoring**: Scouter can be used to monitor your model's performance over time with simple statistics based on process control methodology that is widely used in manufacturing and other industries ([link](https://www.itl.nist.gov/div898/handbook/pmc/section3/pmc31.htm)).


## Current Limitations

- Scouter currently supports 2D arrays that are typically found with tabular data. Support for higher dimensional arrays as seen in computer vision and NLP workloads is planned for future releases.

## Integration with Opsml

While Scouter is a standalone model monitoring and data profiling service, it was originally designed to integrate with `opsml`. This integration is currently ongoing and will be released in the future.

## Getting Started

### Installation

```bash
pip install scouter-ml
```

### Usage for Model Monitoring


```python
from scouter import Drifter, DriftConfig, DriftMap

# Get data
data = generate_data()

# Initialize Drifter
drifter = Drifter()

# Create DriftConfig
config = DriftConfig(name="model", repository="scouter", version="0.1.0")

# Create Drift profile
profile = drifter.create_drift_profile(data, config)
print(profile)

```

```json
{
  "features": {
    "feature_1": {
      "id": "feature_1",
      "center": -3.9505726402457264,
      "one_ucl": -1.9357578944262643,
      "one_lcl": -5.9653873860651885,
      "two_ucl": 0.07905685139319774,
      "two_lcl": -7.980202131884651,
      "three_ucl": 2.0938715972126594,
      "three_lcl": -9.995016877704112,
      "timestamp": "2024-06-26T20:43:27.957232"
    },
    "feature_2": {
      "id": "feature_2",
      "center": -3.8967421987245485,
      "one_ucl": -1.8347509279483476,
      "one_lcl": -5.95873346950075,
      "two_ucl": 0.22724034282785333,
      "two_lcl": -8.02072474027695,
      "three_ucl": 2.2892316136040547,
      "three_lcl": -10.08271601105315,
      "timestamp": "2024-06-26T20:43:27.957233"
    },
    "feature_3": {
      "id": "feature_3",
      "center": -4.139930289046504,
      "one_ucl": -2.0997890884791675,
      "one_lcl": -6.18007148961384,
      "two_ucl": -0.05964788791183118,
      "two_lcl": -8.220212690181176,
      "three_ucl": 1.980493312655505,
      "three_lcl": -10.260353890748512,
      "timestamp": "2024-06-26T20:43:27.957150"
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
    "name": "test_app",
    "repository": "statworld",
    "version": "0.1.0",
    "alert_config": {
        "alert_dispatch_type": "Console",
        "schedule": "0 0 0 * * *",
        "alert_rule": {
              "process": {
                "rule": "4 4 4 8 2 4 1 1",
                "zones_to_monitor": [
                  "Zone 1", 
                  "Zone 2", 
                  "Zone 3", 
                  "Zone 4"
                ]
              },
              "percentage": null
        },
        "features_to_monitor": [],
        "alert_kwargs": {}
    },
    "feature_map": null,
    "targets": []
  },
  "scouter_version": "0.1.0"
}
```


The drift profile can then be sent to the `scouter-server` for monitoring and alerting. However, monitoring and alerting can also be done locally.

```python
new_data = generate_new_data()

# Check for drift (use the same profile)
drift_map: DriftMap = drifter.compute_drift(data, profile)

# alert generation requires numpy arrays
drift_array, sample_array, features = drift_map.to_numpy()

alerts = drifter.generate_alerts(drift_array, features, profile.config.alert_rule)

print(alerts)
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
      "alerts": [
        {
          "kind": "Consecutive",
          "zone": "Zone 1"
        }
      ],
      "indices": {
        "1": [
          [
            28,
            36
          ]
        ]
      }
    },
    "feature_3": {
      "feature": "feature_3",
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

