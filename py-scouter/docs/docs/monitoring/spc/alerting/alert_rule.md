# SPC Alert Configuration

---

!!! info "scouter.alert.SpcAlertRule"
Configure how to detect drift using `SpcAlertRule`.

---

```py
from scouter.alert import SpcAlertRule, AlertZone

SpcAlertRule(
    rule="8 16 4 8 2 4 1 1",
    zones_to_monitor=[AlertZone.Zone1, AlertZone.Zone2, AlertZone.Zone3, AlertZone.Zone4]
)
```

## Parameters

| Parameter           | Type              | Description                                                                 | Example                                 |
|---------------------|-------------------|-----------------------------------------------------------------------------|-----------------------------------------|
| rule                | `str`             | Rule to use for alerting. Eight digit integer string. Defaults to '8 16 4 8 2 4 1 1 | `alert_rule.rule -> "8 16 4 8 2 4 1 1"` |
| zones_to_monitor    | `list[AlertZone]` |  List of zones to monitor. Defaults to all zones.                  | `alert_rule.zones → [AlertZone.Zone1, AlertZone.Zone2]`                 |

## Properties

| Property              | Type        | Description                                                                         | Example                              |
|-----------------------|-------------|-------------------------------------------------------------------------------------|--------------------------------------|
| rule                | `str`             | Rule to use for alerting. Eight digit integer string. Defaults to '8 16 4 8 2 4 1 1 | `alert_rule.rule -> "8 16 4 8 2 4 1 1"` |
| zones_to_monitor    | `list[AlertZone]` |  List of zones to monitor. Defaults to all zones.                  | `alert_rule.zones → [AlertZone.Zone1, AlertZone.Zone2]`                 |