
While Scouter comes packed with powerful drift detection tools, we understand that no single solution fits every use case. You might find yourself needing a drift detection method that isn’t natively supported—don’t worry, we’ve got you covered. Scouter provides built-in support for custom metric tracking, allowing you to define your own metrics and baseline values. We’ll handle the heavy lifting of saving inference data and detecting drift over time.

- Setting up a Custom drift profile
- Configuring real-time notifications for model drift detection
- Using scouter queues and fastapi integrations allowing you to send data to scouter server at the time of model inference

### Creating a Drift Profile
To detect model drift, we first need to create a drift profile using your baseline dataset, this is typically done at the time of training your model.
```python
from scouter.alert import SlackDispatchConfig
from scouter.client import ScouterClient
from scouter.drift import Drifter, CustomMetric, CustomMetricDriftConfig, CommonCrons
from sklearn import datasets

if __name__ == "__main__":

    # Define custom metrics
    metrics = [  #(1)
        CustomMetric(
            name="mae",
            value=1,
            alert_threshold=AlertThreshold.Outside,
            alert_threshold_value=0.5,
        ),
        CustomMetric(
            name="mape",
            value=2,
            threshold=AlertThreshold.Outside,
            delta=0.5,
        ),
    ]

    # Define the drift config
    drift_config = CustomMetricDriftConfig(
        space="scouter",
        name="custom_metric",
        version=semver,
        sample_size=25,  #(2)
        alert_config=CustomMetricAlertConfig(
            schedule="*/5 * * * * *",  # every 5 minutes
        ),
    )

    # Drifter class for creating drift profiles
    drifter = Drifter()

    profile = drifter.create_drift_profile(data=metrics, config=drift_config)
    client.register_profile(profile)
```

1. Instead of data, you provide the Drifter with a list of `CustomMetric` objects. Each metric has a name, value, and alert threshold.
2. The sample size is the number of samples to use for drift detection. This is the number of samples that will be used to calculate the drift score. The default value is 25, but you can adjust this based on your needs. It is generally recommended to not use a sample size of less than 25, as drift alerting is computed from the sampled value. If sample size is 1, an alert will be triggered every time the value surpasses the defined threshold.

## CustomMetric

The `CustomMetric` class is used to define a custom metric for drift detection. It contains the following properties:

| Argument      | Type    | Description |
| ----------- | --------- | ----------- |
| `name`       | string | The name to assign to the metric |
| `value`       | float | The value of the metric |
| `alert_threshold` | `AlertThreshold` | The condition used to determine when an alert should be triggered. |
| `alert_threshold_value` | float (optional) | The threshold value for the alert. This is the value that will be used to determine if an alert should be triggered. |


### AlertThreshold

Enum representing different alert conditions for monitoring metrics.

| Enum Value | Description |
| ----------- | ----------- |
| `Below`    | Indicates that an alert should be triggered when the metric is below a threshold
| `Above`   | Indicates that an alert should be triggered when the metric is above a threshold. |
| `Outside`     | Indicates that an alert should be triggered when the metric is outside a specified range. |


### CustomMetricDriftConfig
The `CustomMetricDriftConfig` class is used to define the configuration for a custom metric drift profile. It contains the following properties:

| Argument      | Type    | Description |
| ----------- | --------- | ----------- |
| `space`       | string | The name of the model space |
| `name`       | string | The name of the model |
| `version`       | string | The version of the model |
| `sample_size` | int | The number of samples to use for drift detection. This is the number of samples that will be used to calculate the drift score. The default value is 25, but you can adjust this based on your needs. It is generally recommended to not use a sample size of less than 25, as drift alerting is computed from the sampled value. If sample size is 1, an alert will be triggered every time the value surpasses the defined threshold |
| `alert_config` | `CustomMetricAlertConfig` | The alert configuration for the drift profile. This is an instance of the `CustomMetricAlertConfig` class. |


### CustomMetricAlertConfig
The `CustomMetricAlertConfig` class is used to define the alert configuration for a custom metric drift profile. It contains the following properties:

| Argument      | Type    | Description |
| ----------- | --------- | ----------- |
| `schedule`       | string | The schedule for the drift detection job. This is a cron expression that defines when the job should run. |
| `dispatch_config` | `DispatchConfig` | The dispatch configuration for the drift profile. This is an instance of the `DispatchConfig` class. |