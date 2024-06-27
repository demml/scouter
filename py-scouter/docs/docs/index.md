<h1 align="center">
  <br>
  <img src="https://github.com/demml/scouter/blob/main/images/scouter-logo.png?raw=true"  width="600"alt="scouter logo"/>
  <br>
</h1>

<h4 align="center">Observability for Machine Learning</h4>

[![gitleaks](https://img.shields.io/badge/protected%20by-gitleaks-purple)](https://github.com/zricethezav/gitleaks-action)

## **What is it?**

`Scouter` is a model monitoring and data profiling library for machine learning workflows. It is designed to be simple, fast, and easy to use.

## **Why use it?**

Observability remains a challenge for all machine learning projects due to the complexity of needs and lack of tooling and functionality. `Scouter` was built on the following principles:

- **Make it fast**: The core logic is written in `Rust` and exposed as either a rust crate or a Python package via Maturin and PyO3. This means super fast array processing and asynchronous execution for small and large datasets. In addition, the sister project `scouter-server` provides a centralized monitoring and alerting server for your models. This is still in development and not yet ready for production use, so stay tuned!!!
- **Make it simple**: Most industries have been doing some sort of monitoring and alerting for decades. While machine learning is new to the space, it's not inventing it. Monitoring, alerting and profiling should rely on tried and true methods that are easy to understand, easily implemented and have been battle tested. Out of the box, `Scouter` leverages process control methodology that is widely used in manufacturing and other industries ([link](https://www.itl.nist.gov/div898/handbook/pmc/section3/pmc31.htm)).
- **Make it easy to use**: Creating a monitoring configuration and profile should be easy to add to any workflow and shouldn't clog up the codebase and compute runtime (see 'make it fast').


## Getting Started

### Installation

```bash
pip install scouter
```

## Usage

Refer to the different sections for usage:

- [Model Monitoring](./monitoring.md)
- [Data Profiling](./profiling.md)

