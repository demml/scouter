# Drift Configuration

**All models that create a `DriftProfile` will require a `DriftConfig` object. This object is used to configure the drift detection algorithm and alerting system.**

The `DriftConfig` object has the following structure:

```json
{"name": "model",
 "repository": "scouter",
 "version": "0.1.0",
 "sample_size": 100,
 "sample": true,
 "alert_config": {
     "alert_rule": {
         "process_rule": {"rule": "16 32 4 8 2 4 1 1"}
     },
     "alert_dispatch_type": "Console",
     "schedule": "0 0 0 0 0"
 }
}
```

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

`alert_dispatch_type`
: The type of alerting to use. Defaults to `AlertDispatchType.Console`. See [Alerting](./alerting.md) for more information.

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
from scouter import SpcDriftConfig, EveryDay, CommonCrons


# Recommended way to set the schedule
config = SpcDriftConfig(
    name="model",
    repository="scouter",
    version="0.1.0",
    schedule=CommonCrons.EVERY_DAY
)

# This will also work
config = SpcDriftConfig(
    name="model",
    repository="scouter",
    version="0.1.0",
    schedule=EveryDay().cron
)


# or your own
config = SpcDriftConfig(
    name="model",
    repository="scouter",
    version="0.1.0",
    schedule="0 0 0 * * *"
)
```


::: scouter.drift.SpcDriftConfig
    options:
        show_root_heading: true
        heading_level: 3