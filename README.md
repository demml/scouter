<h1 align="center">
  <br>
  <img src="https://github.com/demml/scouter/blob/main/images/scouter-logo.png?raw=true"  width="1181" alt="scouter logo"/>
  <br>
</h1>

<h2 align="center"><b>Observability for Machine Learning</b></h2>


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
pip install scouter
```

### Usage for Model Monitoring


```python
from scouter import Drifter, DriftConfig, DriftMap

# Get data
data = generate_data()

# Initialize Drifter
drifter = Drifter()

# Create DriftConfig
config = DriftConfig(
        name="model",
        repository="scouter",
        version="0.1.0",
    )


# Create Drift profile
profile = drifter.create_drift_profile(data, config)
print(profile)

```

```json
{
  "features": {
    "col_5": {
      "id": "col_5",
      "center": -3.9505726402457264,
      "one_ucl": -1.9357578944262643,
      "one_lcl": -5.9653873860651885,
      "two_ucl": 0.07905685139319774,
      "two_lcl": -7.980202131884651,
      "three_ucl": 2.0938715972126594,
      "three_lcl": -9.995016877704112,
      "timestamp": "2024-06-26T20:43:27.957232"
    },
    "col_7": {
      "id": "col_7",
      "center": -3.8967421987245485,
      "one_ucl": -1.8347509279483476,
      "one_lcl": -5.95873346950075,
      "two_ucl": 0.22724034282785333,
      "two_lcl": -8.02072474027695,
      "three_ucl": 2.2892316136040547,
      "three_lcl": -10.08271601105315,
      "timestamp": "2024-06-26T20:43:27.957233"
    },
    "col_0": {
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
    "col_11": {
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
    "col_9": {
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
    },
    "col_4": {
      "id": "col_4",
      "center": -4.048275753608167,
      "one_ucl": -2.002464448095417,
      "one_lcl": -6.094087059120918,
      "two_ucl": 0.04334685741733324,
      "two_lcl": -8.139898364633668,
      "three_ucl": 2.089158162930084,
      "three_lcl": -10.18570967014642,
      "timestamp": "2024-06-26T20:43:27.957231"
    },
    "col_8": {
      "id": "col_8",
      "center": -4.0158096148958595,
      "one_ucl": -2.0282304547290098,
      "one_lcl": -6.003388775062709,
      "two_ucl": -0.040651294562160434,
      "two_lcl": -7.990967935229559,
      "three_ucl": 1.946927865604689,
      "three_lcl": -9.978547095396408,
      "timestamp": "2024-06-26T20:43:27.957234"
    },
    "col_2": {
      "id": "col_2",
      "center": -4.090610111507429,
      "one_ucl": -2.146102177058493,
      "one_lcl": -6.035118045956365,
      "two_ucl": -0.20159424260955694,
      "two_lcl": -7.9796259804053005,
      "three_ucl": 1.7429136918393793,
      "three_lcl": -9.924133914854238,
      "timestamp": "2024-06-26T20:43:27.957229"
    },
    "col_6": {
      "id": "col_6",
      "center": -4.096466199184098,
      "one_ucl": -2.0563240571792822,
      "one_lcl": -6.136608341188914,
      "two_ucl": -0.016181915174466432,
      "two_lcl": -8.176750483193729,
      "three_ucl": 2.023960226830349,
      "three_lcl": -10.216892625198545,
      "timestamp": "2024-06-26T20:43:27.957232"
    },
    "col_1": {
      "id": "col_1",
      "center": -3.997113080300062,
      "one_ucl": -1.9742384896265417,
      "one_lcl": -6.019987670973582,
      "two_ucl": 0.048636101046978464,
      "two_lcl": -8.042862261647102,
      "three_ucl": 2.071510691720498,
      "three_lcl": -10.065736852320622,
      "timestamp": "2024-06-26T20:43:27.957229"
    },
    "col_3": {
      "id": "col_3",
      "center": -3.937652409303277,
      "one_ucl": -2.0275656995100224,
      "one_lcl": -5.8477391190965315,
      "two_ucl": -0.1174789897167674,
      "two_lcl": -7.757825828889787,
      "three_ucl": 1.7926077200764872,
      "three_lcl": -9.66791253868304,
      "timestamp": "2024-06-26T20:43:27.957230"
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
    "col_11": {
      "feature": "col_11",
      "alerts": [],
      "indices": {}
    },
    "col_3": {
      "feature": "col_3",
      "alerts": [],
      "indices": {}
    },
    "col_9": {
      "feature": "col_9",
      "alerts": [],
      "indices": {}
    },
    "col_2": {
      "feature": "col_2",
      "alerts": [],
      "indices": {}
    },
    "col_1": {
      "feature": "col_1",
      "alerts": [],
      "indices": {}
    },
    "col_0": {
      "feature": "col_0",
      "alerts": [],
      "indices": {}
    },
    "col_8": {
      "feature": "col_8",
      "alerts": [],
      "indices": {}
    },
    "col_4": {
      "feature": "col_4",
      "alerts": [],
      "indices": {}
    },
    "col_5": {
      "feature": "col_5",
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
    "col_6": {
      "feature": "col_6",
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
    "col_7": {
      "feature": "col_7",
      "alerts": [],
      "indices": {}
    },
    "target": {
      "feature": "target",
      "alerts": [],
      "indices": {}
    }
  }
}
```

