from scouter import (
    Drifter,
    DriftConfig,
    AlertRule,
    ProcessAlertRule,
    DriftMap,
    AlertDispatchType,
)
from .utils import generate_data


if __name__ == "__main__":
    # generate data
    data = generate_data()

    # create drift config (usually associated with a model name, repository name, version)
    config = DriftConfig(
        name="model",
        repository="scouter",
        version="0.1.0",
        alert_rule=AlertRule(  # alert_rule is optional and will default to a standard process alert rule
            process_rule=ProcessAlertRule(),
        ),
        alert_dispatch_type=AlertDispatchType.Console,
    )

    # create drifter
    drifter = Drifter()

    # create drift profile
    profile = drifter.create_drift_profile(data, config)

    # print drift profile
    print(profile)

    # compute drift
    drift_map: DriftMap = drifter.compute_drift(data, profile)

    drift_array, features = drift_map.to_py()

    print(drift_array, features)

    feature_alerts = drifter.generate_alerts(
        drift_array, features, profile.config.alert_config.alert_rule
    )

    print(feature_alerts)
