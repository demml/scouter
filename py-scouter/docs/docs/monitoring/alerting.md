# Alerting

As mentioned, Scouter's monitoring and alerting methodology is based on statistical process control.


## Statistical Process Control ([reference](https://www.itl.nist.gov/div898/handbook/pmc/section1/pmc12.htm))

The basic idea of statistical process control is to compare current process output/data to previous/expected output/data. To do this from an modeling perspective, we take a snapshot of the model's data and predictions at the time of model creation and create a `DriftProfile`. This is done by sampling the data and calculating a series of means and standard deviations in order to approximate the population distribution. From this grand mean and standard deviation, we can calculate the upper and lower control limits for the data.

Reference for grand mean and standard deviation calculations: [link](https://www.itl.nist.gov/div898/handbook/pmc/section3/pmc321.htm)


## Formulas 

Sample mean and standard deviation:

$$
\overline{x}_i = \frac{1}{n}\sum_{i=1}^n x_i
$$

$$
\sigma = \sqrt{\frac{1}{n}\sum_{i=1}^n(x_i-\overline{x})^2}
$$


Grand sample mean and standard deviation:

$$
\overline{\overline{x}}=\frac{1}{n}\sum_{i=1}^n\overline{x}_i
$$


$$
\overline{s}=\frac{1}{n}\sum_{i=1}^n{\sigma}_i
$$

$$
\hat\sigma{}=\frac{\overline{s}}{c_4}
$$

Where $c_4$ is the bias correction factor for the sample standard deviation. The bias correction factor is given by:

$$
c_4 = \sqrt{\frac{2-1}{n}} {\frac{(\frac{n}{2} - 1)!}{(\frac{n-1}{2} - 1)!}}
$$


Control limits:

$$
UCL = \overline{\overline{x}} + k\hat\sigma{}
$$

$$
LCL = \overline{\overline{x}} - k\hat\sigma{}
$$

Where $k$ is the number of standard deviations from the grand mean. Typically, $k=3$ is used for the upper and lower control limits.
**Note**: the calculation of control limits in `Scouter` uses $\hat\sigma{}$. In some text books you will see $\frac{\hat\sigma{}}{\sqrt{n}}$. `Scouter` uses the former in order to widen the control limits and reduce false positives.

## Control Limits

Out of the box, `Scouter` will calculate the center line and control limits for 3 zones ($\pm{1}$, $\pm{2}$ and $\pm{3}$). This is based on the 3 sigma rule in process control. The resulting chart and zones would appear as follows if plotted:

<h1 align="center">
  <br>
  <img src="../../images/control_chart.png"  width="700"alt="scouter logo"/>
  <br>
</h1>

Each dot on the chart represents the mean of a sample from the process being monitored

Zone specifications are as follows:

- Zone 1: 1 LCL -> 1 UCL
- Zone 2: 2 LCL -> 2 UCL
- Zone 3: 3 LCL -> 3 UCL

## Alerting

When new data comes in, we can compare the new data to calculated control limits to see if the process is stable. If it's not, we can generate alerts.
So what does stable mean? In process control, a process is considered stable if the data points fall within the control limits and follow a non-repeating patten. There is great room for flexibility in this definition, which allows data scientists and engineers to customize their alerts rules.

By default, `Scouter` follows an 8 digit rule for process control. The default rule follows [WECO](https://en.wikipedia.org/wiki/Western_Electric_rules) rules with some modification, which is defined as **"8 16 4 8 2 4 1 1"**.

### Rule breakdown

`First digit`
: Zone 1 - 8 points in n + 1 observations on one side of the center line in Zone 1 or greater

`Second digit`
: Zone 1 - 16 points in n + 1 observations on alternating sides of the center line in Zone 1 or greater

`Third digit`
: Zone 2 - 4 points in n + 1 observations on one side of the center line in Zone 2 or greater

`Fourth digit`
: Zone 2 - 8 points in n + 1 observations on alternating sides of the center line in Zone 2 or greater

`Fifth digit`
: Zone 3 - 2 points in n + 1 observations on one side of the center line in Zone 3 or greater

`Sixth digit`
: Zone 3 - 4 points in n + 1 observations on alternating sides of the center line in Zone 3 or greater

`Seventh digit`
: Zone 4 (Out of Control) - 1 points in n + 1 observations on one side of the center line greater than Zone 3

`Eighth digit`
: Zone 4 (Out of Control) - 1 points in n + 1 observations on alternating sides of the center line greater than Zone 3


In addition to the 8 digit rule, `Scouter` will also check for a consecutive trend of 7 increasing or decreasing points and return an alert if one is detected.

### Example Alert

<h1 align="center">
  <br>
  <img src="../../images/control_chart_alert.png"  width="700"alt="scouter logo"/>
  <br>
</h1>

### Custom Alerts

`Scouter` provides the ability to create your own custom 8 digit rules. Below is an example of how to create a custom rule:

```python hl_lines="6 15"
from scouter import DriftConfig, AlertRule, ProcessAlertRule, AlertDispatchType

# Create a custom rule
custom_rule = AlertRule(
    process=ProcessAlertRule(
        rule="16 32 4 8 2 4 1 1" # create your custom rule here
    ),
)

# Create a drift config
config = DriftConfig(
    name="model",
    repository="scouter",
    version="0.1.0",
    alert_rule=custom_rule,
    alert_dispatch_type=AlertDispatchType.Console,
)
```


### Percentage Alerting

In addition to the 8 digit rule, `Scouter` also provides the ability to alert on percentage drift. This is useful when you want to know if a feature has drifted by a certain percentage. The default percentage rule is .10 (10%).