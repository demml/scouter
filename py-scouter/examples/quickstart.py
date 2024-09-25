from scouter import (
    Drifter,
    DriftConfig,
    AlertRule,
    ProcessAlertRule,
    DriftMap,
    AlertDispatchType,
    MonitorStrategy,
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
        monitor_strategy=MonitorStrategy.ProcessControl
    )

    # create drifter
    drifter = Drifter(config)

    # create drift profile
    profile = drifter.create_drift_profile(data)

    # print drift profile
    print(profile)

    # compute drift
    drift_map: DriftMap = drifter.compute_drift(data, profile)

    drift_array, sample_array, features = drift_map.to_numpy()

    print(drift_array, features)

    feature_alerts = drifter.generate_alerts(
        drift_array, features, profile.config.alert_config.alert_rule
    )

    print(feature_alerts)
