<h1 align="center">

</h1>
## Population Stability Index([reference](https://scholarworks.wmich.edu/cgi/viewcontent.cgi?article=4249&context=dissertations))


## Introduction

Population Stability Index (PSI) is a statistical metric used to measure the distribution shift between two datasets. It is commonly used in model monitoring to detect data drift over time. A high PSI value indicates a significant change in the data distribution, potentially signaling model degradation.


## Why Use PSI?

PSI is particularly useful in machine learning and data science for:


- Monitoring model input distributions to detect data drift.

- Validating that training and production data remain similar.

- Ensuring model predictions remain stable over time.

- Detecting changes in customer behavior, economic conditions, or market trends.


## Mathematical Definition of PSI

PSI is calculated using the formula:


$$
\text{PSI}(Y_b, Y, B) = \sum_{i=1}^{B} \left( y_i - y_{b_i} \right) \ln\left( \frac{y_i}{y_{b_i}} \right)
$$


where:

- $y_1, . . . , y_B$ are the proportions of the $i_{th}$ bin collected during the time of inference

- $y_{b_1}, . . . , y_{b_B}$ are the proportions of the $i_{th}$ bin collected during the time of training

- $B$ represents number of bins


## Interpreting PSI Values

The rule of thumb for interpreting PSI values is:

| **PSI Value** | **Interpretation**                |
|---------------|-----------------------------------|
| `< 0.1`       | No significant change             |
| `0.1 - 0.25`  | Moderate shift, monitor closely   |
| `> 0.25`      | Significant shift, investigate    |

## How PSI Works

1. **Bin the Data**: Define bins (e.g., equal-width or quantile-based) for both the expected and observed distributions.
2. **Calculate Proportions**: Compute \( p_i \) and \( q_i \) for each bin.
3. **Apply PSI Formula**: Sum the PSI contributions across all bins.
4. **Analyze Results**: If PSI is high, investigate the underlying cause of the drift.

## Example Calculation

Consider an expected distribution and an observed distribution with the following bin frequencies:

| **Bin Range** | **Expected Count** | **Observed Count** |
|---------------|--------------------|--------------------|
| `0-10`        | `1000`             | `800`              |
| `10-20`       | `1500`             | `1400`             |
| `20-30`       | `1200`             | `1600`             |
| `30-40`       | `1300`             | `1200`             |

Convert these into proportions:

| **Bin Range** | **Expected Proportion**  | **Observed Proportion**  |
|---------------|--------------------------------------|--------------------------------------|
| `0-10`        | `0.2`                                | `0.16`                               |
| `10-20`       | `0.3`                                | `0.28`                               |
| `20-30`       | `0.24`                               | `0.32`                               |
| `30-40`       | `0.26`                               | `0.24`                               |

Applying the PSI formula:

$$PSI = (0.2 - 0.16) \ln\left(\frac{0.2}{0.16}\right) + (0.3 - 0.28) \ln\left(\frac{0.3}{0.28}\right) + (0.24 - 0.32) \ln\left(\frac{0.24}{0.32}\right) + (0.26 - 0.24) \ln\left(\frac{0.26}{0.24}\right)$$

## Binning Strategies

Currently, scouter PSI supports the decile binning approach, which is widely recognized as an industry standard and has shown to provide optimal performance in most use cases.  
We are actively working on expanding the library to support additional binning strategies, offering more flexibility to handle various scenarios.


## Conclusion

PSI is a powerful tool for detecting data drift in production models. By regularly monitoring PSI values, data scientists and engineers can proactively maintain model performance and stability.