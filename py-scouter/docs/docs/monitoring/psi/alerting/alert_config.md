# PSI Alert Configuration

---

!!! info "scouter.alert.PsiAlertConfig"
Configure how you and your team want to be alerted in the event of model drift using `PsiAlertConfig`.

---

```py
from scouter.alert import PsiAlertConfig, SlackDispatchConfig
from scouter.types import CommonCrons

PsiAlertConfig(
    dispatch_config=SlackDispatchConfig(channel='my-team-channel'),
    schedule=CommonCrons.Every6Hours,
    features_to_monitor=['feature_1', 'feature_2', ...],
)
```

## Parameters

| Parameter           | Type                 | Description                                                                   | Example                                                      |
|---------------------|----------------------|-------------------------------------------------------------------------------|--------------------------------------------------------------|
| dispatch_config     | `SlackDispatchConfig | OpsGenieDispatchConfig                                                        | None`                                                        | An optional dispatch configuration used to configure how alerts are routed, if None is provided a default internal dispatch type of Console will be used to log alerts to the conosole of the scouter server.  | `config.dispatch_config -> SlackDispatchConfig()` |
| schedule            | `str                 | CommonCrons                                                                   | None`                                                        | Schedule to run drift detection job. Defaults to daily at midnigh. You can use the builtin CommonCron options or specify your own custom cron.                  | `config.schedule → CommonCrons.Every6Hours` |
| features_to_monitor | `list[str]`                                        | List of features to monitor. Defaults to empty list, which means all features | `config.features_to_monitor → ['feature_1, feature_2, ...']` |
| psi_threshold        | `float`              | Defaults to the industry standard of 0.25. If one of your monitored features surpass the psi_threshold, an alert will be sent.                                   | `config.psi_threshold → 0.25`                                |

## Properties

| Property              | Type        | Description                                                                         | Example                              |
|-----------------------|-------------|-------------------------------------------------------------------------------------|--------------------------------------|
| `dispatch_type`       | `str`       | String representation of what type of dispatch are you using to send alerts.        | `config.dispatch_type` → `"Slack"`   |
| `dispatch_Config`     | `SlackDispatchConfig | OpsGenieDispatchConfig                                                        | None`      | Dispatch configuration used to configure how alerts are routed.                                 | `config.dispatch_config -> SlackDispatchConfig()`   |
| `schedule`            | `str`       | The schedule that is used to determine when your drift detecion job should run.     | `config.schedule` → `"0 0 0 * * SUN"` |
| `features_to_monitor` | `list[str]` | List of features to monitor.                                                        | `config.features_to_monitor → ['feature_1, feature_2, ...']`        |
| `psi_threshold`       | `float`     | If one of your monitored features surpass the psi_threshold, an alert will be sent. | `config.psi_threshold → 0.25`         |

