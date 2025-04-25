# pylint: disable=invalid-name
from pathlib import Path
import numpy as np
import pandas as pd
from scouter import CommonCrons, Drifter, PsiDriftConfig
from scouter.alert import AlertThreshold, PsiAlertConfig
from scouter.client import ScouterClient


def generate_data() -> pd.DataFrame:
    """Create a fake data frame for testing"""
    n = 10_000
    X_train = np.random.normal(-4, 2.0, size=(n, 4))
    col_names = []
    for i in range(0, X_train.shape[1]):
        col_names.append(f"col_{i}")
    X = pd.DataFrame(X_train, columns=col_names)
    return X


def create_psi_profile() -> Path:
    """Create a PSI profile

    The following example shows how to:

    1. Instantiate the Drifter class and connect to the Scouter client
    2. Create a fake data frame
    3. Create a PSI profile using the Drifter class
    4. Register the profile with the Scouter client and set it as active
    (this will tell the server to schedule the profile for alerting)
    5. Save the profile to a json file (we'll use this to load it in the api for demo purposes)
    """
    # Drifter class for creating drift profiles
    drifter = Drifter()

    # Simple client to register drift profiles (scouter client must be running)
    client = ScouterClient()

    # create fake data
    data = generate_data()

    # create psi configuration
    psi_config = PsiDriftConfig(
        space="scouter",
        name="psi_test",
        version="0.0.1",
        features_to_monitor=["feature_1", "feature_2"],
        alert_config=PsiAlertConfig(
            schedule=CommonCrons.Every6Hours,  # You can also use a custom cron expression
        ),
    )

    # create psi profile
    psi_profile = drifter.create_drift_profile(data, psi_config)

    # register profile
    client.register_profile(profile=psi_profile, set_active=True)

    # save profile to json (for example purposes)
    return psi_profile.save_to_json()


if __name__ == "__main__":
    # Create a PSI profile and get its path
    profile_path = create_psi_profile()

    print(f"Profile saved to {profile_path}")
