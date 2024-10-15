from scouter import (
    Drifter,
    SpcDriftConfig,
    DriftType, PsiDriftConfig, PsiDriftMap,
)
from utils import generate_data


if __name__ == "__main__":
    # generate data
    data = generate_data()

    # create drift config (usually associated with a model name, repository name, version)
    config = PsiDriftConfig(
        name="model",
        repository="scouter",
        version="0.1.0"
    )

    # create drifter
    drifter = Drifter(DriftType.PSI)
    # create drift profile
    profile = drifter.create_drift_profile(data, config)

    # print drift profile
    data['col_9'] = data['col_9']*100
    data['col_1'] = data['col_9']**2
    # compute drift
    drift_map: PsiDriftMap = drifter.compute_drift(data, profile)

    # drift_array, sample_array, features = drift_map.to_numpy()
    #
    # print(drift_array, features)
    #
    # feature_alerts = drifter.generate_alerts(
    #     drift_array, features, profile.config.alert_config.alert_rule
    # )
    #
    # print(feature_alerts)
