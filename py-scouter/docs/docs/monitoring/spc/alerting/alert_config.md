# SPC Alert Configuration

---

!!! info "scouter.alert.SpcAlertConfig"
Configure how you and your team want to be alerted in the event of model drift using `SpcAlertConfig`.

---

```py
from scouter.alert import SpcAlertConfig, OpsGenieDispatchConfig, SpcAlertRule
from scouter.types import CommonCrons

SpcAlertConfig(
    rule=SpcAlertRule(rule="16 32 4 8 2 4 1 1"),
    dispatch_config=OpsGenieDispatchConfig(team='the-ds-team'),
    schedule=CommonCrons.EveryDay,
    features_to_monitor=['feature_1', 'feature_2', ...],
)
```

## Parameters

| Parameter           | Type                 | Description                                                                   | Example                                                      |
|---------------------|----------------------|-------------------------------------------------------------------------------|--------------------------------------------------------------|
| dispatch_config     | `SlackDispatchConfig | OpsGenieDispatchConfig                                                        | None`                                                        | An optional dispatch configuration used to configure how alerts are routed, if None is provided a default internal dispatch type of Console will be used to log alerts to the conosole of the scouter server.  | `config.dispatch_config -> SlackDispatchConfig()` |
| schedule            | `str                 | CommonCrons                                                                   | None`                                                        | Schedule to run drift detection job. Defaults to daily at midnigh. You can use the builtin CommonCron options or specify your own custom cron.                  | `config.schedule → CommonCrons.Every6Hours` |
| features_to_monitor | `list[str]`                                        | List of features to monitor. Defaults to empty list, which means all features | `config.features_to_monitor → ['feature_1, feature_2, ...']` |
| rule                | `SpcAlertRule`              | Defines the conditions for triggering alerts based on patterns observed in the control chart. Defaults to "8 16 4 8 2 4 1 1", where each digit specifies a threshold for detecting instability within each control zone (Zone 1 to Zone 4). Can be customized for more or less sensitivity.                       | `config.rule → *Instance of SpcAlertRule*`                   |

## Properties

| Property              | Type        | Description                                                                         | Example                              |
|-----------------------|-------------|-------------------------------------------------------------------------------------|--------------------------------------|
| `dispatch_type`       | `str`       | String representation of what type of dispatch are you using to send alerts.        | `config.dispatch_type` → `"Slack"`   |
| `dispatch_Config`     | `SlackDispatchConfig | OpsGenieDispatchConfig                                                        | None`      | Dispatch configuration used to configure how alerts are routed.                                 | `config.dispatch_config -> SlackDispatchConfig()`   |
| `schedule`            | `str`       | The schedule that is used to determine when your drift detecion job should run.     | `config.schedule` → `"0 0 0 * * SUN"` |
| `features_to_monitor` | `list[str]` | List of features to monitor.                                                        | `config.features_to_monitor → ['feature_1, feature_2, ...']`        |
| `rule`       | `SpcAlertRule`     | Defines the conditions for triggering alerts based on patterns observed in the control chart. Defaults to "8 16 4 8 2 4 1 1", where each digit specifies a threshold for detecting instability within each control zone (Zone 1 to Zone 4). Can be customized for more or less sensitivity.                       | `config.rule → *Instance of SpcAlertRule*`                   |

