# pylint: disable=invalid-name

from pathlib import Path

import numpy as np
import pandas as pd
from scouter.alert import (
    AlertThreshold,
    PsiAlertConfig,
    SlackDispatchConfig,
    SpcAlertConfig,
)
from scouter.client import ScouterClient
from scouter.drift import (
    CustomDriftProfile,
    CustomMetric,
    CustomMetricDriftConfig,
    Drifter,
    PsiDriftConfig,
    SpcDriftConfig,
)


def generate_data() -> pd.DataFrame:
    """Create a fake data frame for testing"""
    n = 10_000

    X_train = np.random.normal(-4, 2.0, size=(n, 4))

    col_names = []
    for i in range(0, X_train.shape[1]):
        col_names.append(f"feature_{i}")

    X = pd.DataFrame(X_train, columns=col_names)

    return X


if __name__ == "__main__":

    # Drfter class for creating drift profiles
    scouter = Drifter()

    # Simple client to register drift profiles
    client = ScouterClient()

    # create fake data
    data = generate_data()

    # create psi profile
    psi_config = PsiDriftConfig(
        name="test",
        repository="test",
        version="0.0.1",
        alert_config=PsiAlertConfig(
            features_to_monitor=["feature_1"],
            dispatch_config=SlackDispatchConfig(channel="test_channel"),
        ),
    )

    psi_profile = scouter.create_drift_profile(data, psi_config)

    bins = psi_profile.features["feature_1"].bins

    client.register_profile(psi_profile)
    psi_profile.save_to_json(path=Path("psi_profile.json"))

    # create spc profile
    spc_config = SpcDriftConfig(
        name="test",
        repository="test",
        version="0.0.1",
        alert_config=SpcAlertConfig(
            features_to_monitor=["feature_1"], dispatch_config=SlackDispatchConfig(channel="test_channel")
        ),
    )

    spc_profile = scouter.create_drift_profile(data, spc_config)
    client.register_profile(spc_profile)
    spc_profile.save_to_json(path=Path("spc_profile.json"))

    custom_config = CustomMetricDriftConfig(
        name="test",
        repository="test",
        version="0.0.1",
    )

    custom_profile = CustomDriftProfile(
        config=custom_config,
        metrics=[
            CustomMetric(
                name="mae",
                value=10,
                alert_threshold=AlertThreshold.Above,
            ),
        ],
    )

    client.register_profile(custom_profile)
    custom_profile.save_to_json(path=Path("custom_profile.json"))
