# Drift Configuration

The drift configuration is used to configure the drift profile and is used to configure alert generation. The drift configuration is required when creating a drift profile.


## Arguments

`name`
: The name of the model or dataset you are monitoring.

`repository`
: The repository where the model or dataset is stored.

`version`
: The version of the model or dataset you are monitoring.

`sample`
: Whether to sample the data or not. Defaults to True.

`sample_size`
: The size of the sample to take. Defaults to 25.

`schedule`
: The 6 digit cron schedule for monitoring. Defaults to "0 0 0 * * *".

`alert_rule`
: The alert rule to use for monitoring. Defaults to the 8 digit rule. See [Alerting](./alerting.md) for more information.


## Scheduling

The drift configuration uses a 6 digit cron to schedule monitoring via the `scouter-server`. 

`<second> <minute> <hour> <day of month> <month> <day of week>`

The default value is `0 0 0 * * *` which means to run every day at midnight (UTC).

`Scouter` also includes a few helper classes to that provide common scheduling patterns. These are:

- `Every30Minutes`
- `EveryHour`
- `Every6Hours`
- `Every12Hours`
- `EveryDay`
- `EveryWeek`

```python
from scouter import DriftConfig, EveryDay, CommonCrons


# Recommended way to set the schedule
config = DriftConfig(
    name="model",
    repository="scouter",
    version="0.1.0",
    schedule=CommonCrons.EVERY_DAY
)

# This will also work
config = DriftConfig(
    name="model",
    repository="scouter",
    version="0.1.0",
    schedule=EveryDay().cron
)


# or your own
config = DriftConfig(
    name="model",
    repository="scouter",
    version="0.1.0",
    schedule="0 0 0 * * *"
)
```


::: scouter._scouter.DriftConfig
    options:
        show_root_heading: true
        heading_level: 3