<h1 align="center">
  <br>
  <img src="https://github.com/demml/scouter/blob/main/images/scouter-logo.png?raw=true"  width="600"alt="scouter logo"/>
  <br>
</h1>

<h2 align="center"><b>Observability for Machine Learning</b></h2>

[![gitleaks](https://img.shields.io/badge/protected%20by-gitleaks-purple)](https://github.com/zricethezav/gitleaks-action)

## **What is it?**

`Scouter` is a model monitoring and data profiling library for machine learning workflows. It is designed to be simple, fast, and easy to use.

## **Why use it?**

Observability remains a challenge for all machine learning projects due to the complexity of needs and lack of tooling and functionality. `Scouter` was built on the following principles:

- **Make it fast**: The core logic is written in `Rust` and exposed as a rust crate and Python package via Maturin and PyO3. This means super fast array processing and asynchronous execution for small and large datasets. In addition, the sister project `scouter-server` provides a centralized monitoring and alerting server for your models. This is still in development and not yet ready for production use, so stay tuned!!!

  - **Make it simple**: Most industries have been doing some sort of monitoring and alerting for decades. While machine learning is new to the space, it's not inventing it. Monitoring, alerting and profiling should rely on tried and true methods that are easy to understand, easily implemented and have been battle tested. Out of the box, `Scouter` supports multiple drift detection approaches, including:
      - **Statistical Process Control (SPC)** – A proven method widely used in manufacturing and operations.
      - **Population Stability Index (PSI)** – A standard approach for detecting distribution shifts.
      - **Custom Metrics** – Define your own drift detection method to match your specific needs.

    And we’re not stopping there—**more drift detection approaches are on the way!** Need something custom? Don't sweat it, with Scouter you can specify custom drift detection metrics.

- **Make it easy to use**: Setting up monitoring and profiling for a model should be easy to add to any workflow and shouldn't clog up the codebase and compute runtime (see 'make it fast').

## Concepts

- **Drift Profiles**: Drift profiles are generated during model training to establish a baseline for detecting data drift. Based on your selected drift detection method, specific attributes from the training data are captured and stored in a profile. This profile is then used to compare future inference data, allowing for continuous monitoring of data stability and model performance.
- **Scouter Queues** – Scouter Queues allow you to capture the data being sent to your model during inference. This captured data is then sent to Scouter, where it will be stored and used in the future to detect any potential drift.
- **Alerting** – Based on the drift profile you configure, a scheduled job periodically checks for data drift using the captured inference data. If drift is detected, an alert is triggered and sent via Slack or OpsGenie to notify the relevant team.


## Getting Started

### **Installation**
To install Scouter, simply run:

```bash
pip install scouter
```

### **Configuration**
To register profiles and use Scouter queues, set your company's Scouter server URL as an environment variable:

```bash
export SCOUTER_SERVER_URL=your_scouter_server_url
```

If your company does not support Scouter server, refer your engineering team to the [Scouter Server Setup Guide](#) for installation instructions.  
