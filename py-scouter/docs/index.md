<h1 align="center">
  <br>
  <img src="https://github.com/demml/scouter/blob/main/images/scouter-logo.png?raw=true"  width="600"alt="scouter logo"/>
  <br>
</h1>

<h2 align="center"><b>Quality Control for Machine Learning Monitoring</b></h2>

## **What is it?**

`Scouter` is a developer-first monitoring toolkit for machine learning workflows (data, models, genai workflows and more). It is designed to be easy to use, flexible, performant, and extensible, allowing you to customize it to fit your specific needs. It's built on top of the `Rust` programming language and uses `Postgres` as its primary data store.

## **Why Use It?**

Because you deploy services that need to be monitored, and you want to be alerted when something is wrong.


### Developer-First Experience
- **Zero-friction Integration** - Drop into existing ML workflows in minutes
- **Type-safe by Design** - Rust in the back, Python in the front<sup>*</sup>. Catch errors before they hit production
- **Dependency Overhead** - One dependency for monitoring. No need to install multiple libraries
- **Standardized Patterns** - Out of the box and easy to use patterns for common monitoring tasks
- **Integrations** - Works out of the box with any python api framework. Integrations for event-driven workflows (`Kafka` and `RabbitMQ`)

### Production Ready
- **High-Performance Server** - Built with Rust and Axum for speed, reliability and concurrency
- **Cloud-Ready** - Native support for AWS, GCP, Azure
- **Modular Design** - Use what you need, leave what you don't
- **Alerting and Monitoring** - Built-in alerting integrations with `Slack` and `OpsGenie` to notify you and your team when an alert is triggered
- **Data Retention** - Built-in data retention policies to keep your database clean and performant
  
<sup>
Scouter is written in Rust and is exposed via a Python API built with PyO3.
</sup>

## Quick Start

Scouter follows a client and server architecture whereby the client is a lightweight library that can be dropped into any Python application and the server is a Rust-based service that handles the heavy lifting of data collection, storage, and querying (setup separately).


### Install Scouter
```bash
pip install scouter-ml
```

### Population Stability Index (PSI) Example - Client

```python
import numpy as np
import pandas as pd

from scouter.client import ScouterClient # Get the scouter client in order to interact with the server
from scouter.drift import Drifter, PsiDriftConfig

def generate_data() -> pd.DataFrame:
    """Create a fake data frame for testing"""
    n = 10_000
    X_train = np.random.normal(-4, 2.0, size=(n, 4))
    col_names = []
    for i in range(0, X_train.shape[1]):
        col_names.append(f"col_{i}")
    X = pd.DataFrame(X_train, columns=col_names)
    return X

if __name__ == "__main__":
  # Drfter class for creating drift profiles
  drifter = Drifter()

  client = ScouterClient()

  # get fake data
  data = generate_data()

``# Create a psi config
  psi_config = PsiDriftConfig(
      name="test",
      space="test",
      version="0.0.1",
      features_to_monitor=["feature_1"],
  )

  # Create drift profile
  psi_profile = drifter.create_drift_profile(data, psi_config)

  # register drift profile
  client.register_profile(psi_profile)
```


### Custom Metric Example - Client

```python
import numpy as np
import pandas as pd

from scouter.client import ScouterClient # Get the scouter client in order to interact with the server
from scouter.drift import (
    CustomDriftProfile,
    CustomMetric,
    CustomMetricDriftConfig,
    Drifter,
    PsiDriftConfig,
    SpcDriftConfig,
)

def generate_data() -> pd.DataFrame:
    """Create a fake data frame for testing"""
    n = 10_000
    X_train = np.random.normal(-4, 2.0, size=(n, 4))
    col_names = []
    for i in range(0, X_train.shape[1]):
        col_names.append(f"col_{i}")
    X = pd.DataFrame(X_train, columns=col_names)
    return X

if __name__ == "__main__":
  # Drfter class for creating drift profiles
  drifter = Drifter()

  client = ScouterClient()

  # get fake data
  data = generate_data()

``# Create a custom config
  custom_config = CustomMetricDriftConfig(
      name="test",
      space="test",
      version="0.0.1",
  )

  # Create drift profile
  custom_profile = CustomDriftProfile(
    config=custom_config,
    metrics=[
        CustomMetric(
            name="mae",
            value=10,
            alert_threshold=AlertThreshold.Above, # any value above 10 will trigger an alert
        ),
    ],
  )

  # register drift profile
  client.register_profile(custom_profile)
```